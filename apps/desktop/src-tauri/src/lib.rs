use dawn_detection::classify;
use dawn_security::validate_http_url;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct InspectionResult {
    host: String,
    source_kind: String,
    media_kind: String,
    note: String,
}

#[tauri::command]
fn inspect_url(url: String) -> Result<InspectionResult, String> {
    let parsed = validate_http_url(&url).map_err(|error| error.to_string())?;
    let result = classify(&parsed, None);
    let (source_kind, note) = match result.source_kind {
        dawn_domain::SourceKind::DirectFile => ("direct_file".to_owned(), "This looks like a direct file based on its URL. The server response must still be verified.".to_owned()),
        dawn_domain::SourceKind::HlsManifest => ("hls_manifest".to_owned(), "An HLS manifest was detected. Support depends on stream format, authorization, and protection.".to_owned()),
        dawn_domain::SourceKind::DashManifest => ("dash_manifest".to_owned(), "A DASH manifest was detected. Support depends on stream format, authorization, and protection.".to_owned()),
        _ => ("unknown".to_owned(), "This URL is not identifiable as a direct media file from its path alone.".to_owned()),
    };
    Ok(InspectionResult {
        host: parsed.host_str().unwrap_or("unknown").to_owned(),
        source_kind,
        media_kind: format!("{:?}", result.media_kind).to_ascii_lowercase(),
        note,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![inspect_url])
        .run(tauri::generate_context!())
        .expect("failed to run Your Dawnloader desktop application");
}
