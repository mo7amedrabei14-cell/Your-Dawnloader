use std::{
    env,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use dawn_api_contract::UpsertDownloadRecordRequest;
use dawn_storage::{SessionIdentity, Storage};
use jsonwebtoken::{
    decode, decode_header,
    jwk::{Jwk, JwkSet, KeyAlgorithm, PublicKeyUse},
    Algorithm, DecodingKey, Validation,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;
use url::Url;
use uuid::Uuid;

const SESSION_COOKIE: &str = "dawn_session";
const CSRF_COOKIE: &str = "dawn_csrf";
const SESSION_SECONDS: i64 = 60 * 60 * 24 * 30;
const TOKEN_MAX_BYTES: usize = 16 * 1024;

#[derive(Clone)]
struct AuthConfig {
    issuer: String,
    client_id: String,
    client_secret: String,
    authorization_url: String,
    token_url: String,
    jwks_url: String,
    redirect_url: String,
    app_base_url: String,
    app_origin: String,
    cookie_secure: bool,
    cookie_same_site: String,
}

impl AuthConfig {
    fn from_env() -> anyhow::Result<Self> {
        let required = |key: &str| -> anyhow::Result<String> {
            let value = env::var(key).map_err(|_| anyhow::anyhow!("{key} must be configured"))?;
            if value.trim().is_empty() {
                anyhow::bail!("{key} must not be empty");
            }
            Ok(value)
        };

        let app_base_url = required("APP_BASE_URL")?.trim_end_matches('/').to_owned();
        let parsed_app_url = Url::parse(&app_base_url)?;
        if !matches!(parsed_app_url.scheme(), "http" | "https") || parsed_app_url.host_str().is_none() {
            anyhow::bail!("APP_BASE_URL must be an absolute HTTP(S) URL");
        }
        let app_origin = parsed_app_url.origin().ascii_serialization();
        if app_origin == "null" {
            anyhow::bail!("APP_BASE_URL must have a concrete origin");
        }

        let cookie_secure = env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_owned()).parse::<bool>()?;
        let cookie_same_site = env::var("COOKIE_SAME_SITE").unwrap_or_else(|_| "None".to_owned());
        if !matches!(cookie_same_site.as_str(), "Lax" | "Strict" | "None") {
            anyhow::bail!("COOKIE_SAME_SITE must be Lax, Strict or None");
        }
        if cookie_same_site == "None" && !cookie_secure {
            anyhow::bail!("COOKIE_SAME_SITE=None requires COOKIE_SECURE=true");
        }

        let config = Self {
            issuer: required("OIDC_ISSUER")?,
            client_id: required("OIDC_CLIENT_ID")?,
            client_secret: required("OIDC_CLIENT_SECRET")?,
            authorization_url: required("OIDC_AUTHORIZATION_URL")?,
            token_url: required("OIDC_TOKEN_URL")?,
            jwks_url: required("OIDC_JWKS_URL")?,
            redirect_url: required("OIDC_REDIRECT_URL")?,
            app_base_url,
            app_origin,
            cookie_secure,
            cookie_same_site,
        };

        for (name, value) in [
            ("OIDC_ISSUER", config.issuer.as_str()),
            ("OIDC_AUTHORIZATION_URL", config.authorization_url.as_str()),
            ("OIDC_TOKEN_URL", config.token_url.as_str()),
            ("OIDC_JWKS_URL", config.jwks_url.as_str()),
            ("OIDC_REDIRECT_URL", config.redirect_url.as_str()),
        ] {
            let parsed = Url::parse(value).map_err(|_| anyhow::anyhow!("{name} must be an absolute URL"))?;
            if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
                anyhow::bail!("{name} must be an absolute HTTP(S) URL");
            }
            if parsed.scheme() != "https" && !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1")) {
                anyhow::bail!("{name} must use HTTPS except for local development");
            }
        }

        Ok(config)
    }
}

#[derive(Clone)]
struct JwksCache {
    client: reqwest::Client,
    url: String,
    cached: Arc<RwLock<Option<(Instant, JwkSet)>>>,
}

impl JwksCache {
    fn new(client: reqwest::Client, url: String) -> Self {
        Self { client, url, cached: Arc::new(RwLock::new(None)) }
    }

    async fn key(&self, kid: &str) -> anyhow::Result<Jwk> {
        {
            let guard = self.cached.read().await;
            if let Some((fetched_at, jwks)) = guard.as_ref() {
                if fetched_at.elapsed() < Duration::from_secs(300) {
                    if let Some(key) = jwks.find(kid) {
                        return Ok(key.clone());
                    }
                }
            }
        }

        let jwks: JwkSet = self.client.get(&self.url).send().await?.error_for_status()?.json().await?;
        {
            let mut guard = self.cached.write().await;
            *guard = Some((Instant::now(), jwks.clone()));
        }
        jwks.find(kid).cloned().ok_or_else(|| anyhow::anyhow!("signing key not found"))
    }
}

#[derive(Clone)]
struct AppState {
    storage: Storage,
    config: Arc<AuthConfig>,
    http: reqwest::Client,
    jwks: JwksCache,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}
impl Audience {
    fn contains(&self, expected: &str) -> bool {
        match self {
            Self::One(value) => value == expected,
            Self::Many(values) => values.iter().any(|value| value == expected),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(values) => values.len(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    iss: String,
    sub: String,
    aud: Audience,
    exp: usize,
    iat: usize,
    nonce: String,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    preferred_username: Option<String>,
}
#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
}
#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}
#[derive(Debug, Deserialize)]
struct PreferenceRequest {
    sync_download_history: bool,
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?.split(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        if key.trim() == name { Some(value.trim().to_owned()) } else { None }
    })
}
fn make_cookie(
    config: &AuthConfig,
    name: &str,
    value: &str,
    max_age: i64,
    http_only: bool,
) -> anyhow::Result<HeaderValue> {
    let secure = if config.cookie_secure { "; Secure" } else { "" };
    let http_only = if http_only { "; HttpOnly" } else { "" };
    Ok(HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Max-Age={max_age}; SameSite={}{}{}",
        config.cookie_same_site, http_only, secure
    ))?)
}
fn api_error(status: StatusCode, code: &'static str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error": code})))
}

async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(String, SessionIdentity), (StatusCode, Json<Value>)> {
    let token = cookie_value(headers, SESSION_COOKIE)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "authentication_required"))?;
    let token_hash = sha256_hex(&token);
    let identity = state.storage.session_identity(&token_hash).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "account_lookup_failed"))?
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "session_expired"))?;
    Ok((token_hash, identity))
}

async fn check_csrf(
    state: &AppState,
    headers: &HeaderMap,
    session_hash: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    if headers.get(header::ORIGIN).and_then(|value| value.to_str().ok()) != Some(state.config.app_origin.as_str()) {
        return Err(api_error(StatusCode::FORBIDDEN, "origin_not_allowed"));
    }
    let cookie = cookie_value(headers, CSRF_COOKIE)
        .ok_or_else(|| api_error(StatusCode::FORBIDDEN, "csrf_missing"))?;
    let sent = headers.get("x-csrf-token").and_then(|value| value.to_str().ok())
        .ok_or_else(|| api_error(StatusCode::FORBIDDEN, "csrf_missing"))?;
    if cookie != sent {
        return Err(api_error(StatusCode::FORBIDDEN, "csrf_invalid"));
    }
    let valid = state.storage.validate_session_csrf(session_hash, &sha256_hex(&cookie)).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "csrf_validation_failed"))?;
    if !valid {
        return Err(api_error(StatusCode::FORBIDDEN, "csrf_invalid"));
    }
    Ok(())
}

async fn live() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({"status":"live","service":"dawn-api"})))
}
async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match state.storage.ping().await {
        Ok(()) => (StatusCode::OK, Json(json!({"status":"ready"}))),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status":"not_ready"}))),
    }
}

async fn csrf(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<Value>)> {
    // Preserve a valid session-bound token. If it was lost, rotate the server-side hash.
    if let (Some(session), Some(token)) = (
        cookie_value(&headers, SESSION_COOKIE),
        cookie_value(&headers, CSRF_COOKIE),
    ) {
        let session_hash = sha256_hex(&session);
        let identity = state.storage.session_identity(&session_hash).await
            .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "account_lookup_failed"))?;
        if identity.is_some() {
            let valid = state.storage.validate_session_csrf(&session_hash, &sha256_hex(&token)).await
                .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "csrf_validation_failed"))?;
            if valid {
                return Ok((StatusCode::OK, Json(json!({"csrf_token": token}))).into_response());
            }
            return Err(api_error(StatusCode::FORBIDDEN, "csrf_token_expired"));
        }
    }

    let token = random_token();
    let cookie = make_cookie(&state.config, CSRF_COOKIE, &token, SESSION_SECONDS, false)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "cookie_configuration_error"))?;
    let mut response = (StatusCode::OK, Json(json!({"csrf_token": token}))).into_response();
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}

async fn oidc_login(State(state): State<AppState>) -> Result<Redirect, (StatusCode, Json<Value>)> {
    let oauth_state = random_token();
    let nonce = random_token();
    let verifier = random_token() + &random_token();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    state.storage.create_login_state(&sha256_hex(&oauth_state), &nonce, &verifier).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "login_start_failed"))?;

    let mut url = Url::parse(&state.config.authorization_url)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "oidc_configuration_error"))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &state.config.client_id)
        .append_pair("redirect_uri", &state.config.redirect_url)
        .append_pair("scope", "openid email profile")
        .append_pair("state", &oauth_state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(Redirect::temporary(url.as_str()))
}

async fn oidc_callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    if query.error.is_some() {
        return Err(api_error(StatusCode::UNAUTHORIZED, "identity_provider_login_failed"));
    }
    let code = query.code.filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "authorization_code_missing"))?;
    let raw_state = query.state.filter(|value| !value.is_empty() && value.len() <= 512)
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "oauth_state_missing"))?;
    let login_state = state.storage.consume_login_state(&sha256_hex(&raw_state)).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "login_state_lookup_failed"))?
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "oauth_state_invalid_or_expired"))?;

    let token_response = state.http.post(&state.config.token_url)
        .basic_auth(&state.config.client_id, Some(&state.config.client_secret))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", state.config.redirect_url.as_str()),
            ("code_verifier", login_state.pkce_verifier.as_str()),
        ])
        .send().await.map_err(|_| api_error(StatusCode::BAD_GATEWAY, "token_exchange_failed"))?;
    if !token_response.status().is_success() {
        return Err(api_error(StatusCode::UNAUTHORIZED, "token_exchange_failed"));
    }
    let tokens: TokenResponse = token_response.json().await
        .map_err(|_| api_error(StatusCode::BAD_GATEWAY, "token_response_invalid"))?;
    let id_token = tokens.id_token.filter(|token| token.len() <= TOKEN_MAX_BYTES)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "id_token_missing"))?;

    let token_header = decode_header(&id_token).map_err(|_| api_error(StatusCode::UNAUTHORIZED, "id_token_invalid"))?;
    if token_header.alg != Algorithm::RS256 {
        return Err(api_error(StatusCode::UNAUTHORIZED, "id_token_algorithm_not_allowed"));
    }
    let kid = token_header.kid.filter(|value| !value.is_empty() && value.len() <= 256)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "id_token_key_missing"))?;
    let jwk = state.jwks.key(&kid).await.map_err(|_| api_error(StatusCode::UNAUTHORIZED, "id_token_key_unknown"))?;
    if jwk.common.key_algorithm.is_some_and(|algorithm| algorithm != KeyAlgorithm::RS256)
        || jwk.common.public_key_use.as_ref().is_some_and(|usage| usage != &PublicKeyUse::Signature) {
        return Err(api_error(StatusCode::UNAUTHORIZED, "id_token_key_not_usable"));
    }
    let key = DecodingKey::from_jwk(&jwk).map_err(|_| api_error(StatusCode::UNAUTHORIZED, "id_token_key_invalid"))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[state.config.issuer.as_str()]);
    validation.set_audience(&[state.config.client_id.as_str()]);
    validation.leeway = 30;
    validation.validate_nbf = true;
    let claims = decode::<IdTokenClaims>(&id_token, &key, &validation)
        .map_err(|_| api_error(StatusCode::UNAUTHORIZED, "id_token_invalid"))?.claims;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as usize;
    if claims.iss != state.config.issuer
        || !claims.aud.contains(&state.config.client_id)
        || (claims.aud.len() > 1 && claims.azp.as_deref() != Some(state.config.client_id.as_str()))
        || claims.nonce != login_state.nonce
        || claims.iat > now.saturating_add(60)
        || claims.exp <= now {
        return Err(api_error(StatusCode::UNAUTHORIZED, "id_token_claims_invalid"));
    }

    let email_verified = claims.email_verified.unwrap_or(false);
    let email = if email_verified { claims.email.as_deref() } else { None };
    let display_name = claims.name.as_deref().or(claims.preferred_username.as_deref()).or(email);
    let user_id = state.storage.upsert_oidc_user(
        &claims.iss, &claims.sub, email, email_verified, display_name
    ).await.map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "account_setup_failed"))?;

    let session = random_token() + &random_token();
    let csrf = random_token();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    state.storage.create_session(user_id, &sha256_hex(&session), &sha256_hex(&csrf), now + SESSION_SECONDS).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "session_creation_failed"))?;

    let target = format!("{}/account/downloads", state.config.app_base_url);
    let mut response = Redirect::temporary(&target).into_response();
    response.headers_mut().append(header::SET_COOKIE, make_cookie(&state.config, SESSION_COOKIE, &session, SESSION_SECONDS, true)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "cookie_configuration_error"))?);
    response.headers_mut().append(header::SET_COOKIE, make_cookie(&state.config, CSRF_COOKIE, &csrf, SESSION_SECONDS, false)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "cookie_configuration_error"))?);
    Ok(response)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (_, user) = authenticate(&state, &headers).await?;
    Ok(Json(json!({
        "id": user.user_id,
        "email": user.email,
        "email_verified": user.email_verified,
        "display_name": user.display_name,
        "locale": user.locale,
        "preferences": { "sync_download_history": user.sync_download_history }
    })))
}

async fn update_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PreferenceRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (session, user) = authenticate(&state, &headers).await?;
    check_csrf(&state, &headers, &session).await?;
    state.storage.set_download_sync(user.user_id, request.sync_download_history).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "preference_update_failed"))?;
    Ok(Json(json!({"preferences": {"sync_download_history": request.sync_download_history}})))
}

async fn list_downloads(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (_, user) = authenticate(&state, &headers).await?;
    if !user.sync_download_history {
        return Err(api_error(StatusCode::FORBIDDEN, "download_history_sync_disabled"));
    }
    let page = state.storage.list_download_records(user.user_id, query.limit.unwrap_or(50)).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "download_history_lookup_failed"))?;
    serde_json::to_value(page).map(Json)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "response_serialization_failed"))
}

async fn upsert_download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut request): Json<UpsertDownloadRecordRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (session, user) = authenticate(&state, &headers).await?;
    check_csrf(&state, &headers, &session).await?;
    if !user.sync_download_history {
        return Err(api_error(StatusCode::FORBIDDEN, "download_history_sync_disabled"));
    }
    request.source_domain = request.source_domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if request.source_domain.is_empty() || request.source_domain.len() > 253
        || request.source_domain.chars().any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | ':'))) {
        return Err(api_error(StatusCode::BAD_REQUEST, "source_domain_invalid"));
    }
    if request.display_name.as_ref().is_some_and(|value| value.trim().is_empty() || value.chars().count() > 300)
        || request.provider_item_key.as_ref().is_some_and(|value| value.chars().count() > 256)
        || request.container.as_ref().is_some_and(|value| value.chars().count() > 32)
        || request.quality_label.as_ref().is_some_and(|value| value.chars().count() > 80) {
        return Err(api_error(StatusCode::BAD_REQUEST, "download_record_invalid"));
    }
    if request.completed_at_unix_ms.is_some_and(|value| value < 0) {
        return Err(api_error(StatusCode::BAD_REQUEST, "download_timestamp_invalid"));
    }
    let record = state.storage.upsert_download_record(user.user_id, &request).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "download_record_save_failed"))?;
    serde_json::to_value(record).map(Json)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "response_serialization_failed"))
}

async fn delete_download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let (session, user) = authenticate(&state, &headers).await?;
    check_csrf(&state, &headers, &session).await?;
    let id = Uuid::parse_str(&id).map_err(|_| api_error(StatusCode::NOT_FOUND, "download_record_not_found"))?;
    let deleted = state.storage.delete_download_record(user.user_id, id).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "download_record_delete_failed"))?;
    if deleted { Ok(StatusCode::NO_CONTENT) } else { Err(api_error(StatusCode::NOT_FOUND, "download_record_not_found")) }
}

async fn clear_downloads(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (session, user) = authenticate(&state, &headers).await?;
    check_csrf(&state, &headers, &session).await?;
    let deleted = state.storage.clear_download_records(user.user_id).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "download_history_clear_failed"))?;
    Ok(Json(json!({"deleted_records": deleted})))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let (session, _) = authenticate(&state, &headers).await?;
    check_csrf(&state, &headers, &session).await?;
    state.storage.revoke_session(&session).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "logout_failed"))?;
    let mut response = (StatusCode::OK, Json(json!({"logged_out": true}))).into_response();
    response.headers_mut().append(header::SET_COOKIE, make_cookie(&state.config, SESSION_COOKIE, "", 0, true)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "cookie_configuration_error"))?);
    response.headers_mut().append(header::SET_COOKIE, make_cookie(&state.config, CSRF_COOKIE, "", 0, false)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "cookie_configuration_error"))?);
    Ok(response)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,dawn_api=debug".to_owned()))
        .init();

    let database_url = env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be configured; do not commit it"))?;
    let auth = AuthConfig::from_env()?;
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("YourDawnloader-API/0.1")
        .build()?;
    let jwks = JwksCache::new(client.clone(), auth.jwks_url.clone());
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_str(&auth.app_origin)?)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ORIGIN,
            HeaderName::from_static("x-csrf-token"),
        ]);

    let state = AppState { storage, config: Arc::new(auth), http: client, jwks };
    let app = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/auth/oidc/login", get(oidc_login))
        .route("/auth/oidc/callback", get(oidc_callback))
        .route("/api/v1/csrf", get(csrf))
        .route("/api/v1/me", get(me))
        .route("/api/v1/me/preferences", put(update_preferences))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/downloads", get(list_downloads).post(upsert_download).delete(clear_downloads))
        .route("/api/v1/downloads/{id}", delete(delete_download))
        .with_state(state)
        .layer(cors);

    let address = env::var("API_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse::<std::net::SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    info!(%address, "Your Dawnloader API started with OIDC and per-account download history enabled");
    axum::serve(listener, app).await?;
    Ok(())
}
