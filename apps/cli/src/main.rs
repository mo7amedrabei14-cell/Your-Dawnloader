use clap::{Parser, Subcommand};
use dawn_detection::classify;
use dawn_security::validate_http_url;

#[derive(Debug, Parser)]
#[command(name = "dawn", version, about = "Your Dawnloader diagnostic CLI")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect URL syntax and conservatively classify its likely media kind.
    Inspect { url: String },
}

fn main() {
    let args = Args::parse();
    match args.command {
        Command::Inspect { url } => match validate_http_url(&url) {
            Ok(parsed) => {
                let result = classify(&parsed, None);
                println!("host: {}", parsed.host_str().unwrap_or("unknown"));
                println!("source kind: {:?}", result.source_kind);
                println!("media kind: {:?}", result.media_kind);
                println!("rights: unknown; classification is not authorization");
            }
            Err(error) => {
                eprintln!("URL rejected: {error}");
                std::process::exit(2);
            }
        },
    }
}
