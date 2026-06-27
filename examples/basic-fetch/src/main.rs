//! Fetch a subscription URL and print a normalized profile as JSON.
//!
//! Usage:
//!   cargo run -p basic-fetch -- https://example.com/sub/token
//!
//! Requires the conflux-engine workspace to be built.

use std::env;
use std::process;

#[tokio::main]
async fn main() {
    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            eprintln!("Usage: basic-fetch <subscription-url>");
            eprintln!("Example: basic-fetch https://example.com/sub/token");
            process::exit(1);
        }
    };

    if !url.starts_with("https://") {
        eprintln!("Error: subscription URL must use HTTPS");
        process::exit(1);
    }

    match conflux_engine::fetch_and_normalize(&url).await {
        Ok(subscription) => match serde_json::to_string_pretty(&subscription) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                eprintln!("Error: failed to serialize profile: {err}");
                process::exit(1);
            }
        },
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    }
}
