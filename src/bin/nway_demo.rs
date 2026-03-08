//! N-Way API Conversion Demo

use literbike::api_translation::*;

#[tokio::main]
async fn main() {
    env_logger::init();

    println!("N-Way API Conversion Demo");
    println!("=========================\n");

    println!("=== Detected Providers from Environment ===");
    let mut detected = Vec::new();
    for (key, _) in std::env::vars() {
        if key.ends_with("_API_KEY") || key.ends_with("_AUTH_TOKEN") {
            if let Some(provider) = Provider::from_env_key(&key) {
                println!("  + {:?} ({})", provider, key);
                detected.push(provider);
            }
        }
    }
    println!();

    if detected.is_empty() {
        println!("No API keys detected. Set OPENAI_API_KEY, ANTHROPIC_AUTH_TOKEN, etc.");
        return;
    }

    println!("=== Provider Base URLs ===");
    for provider in &detected {
        println!("  {:?}: {}", provider, provider.base_url());
    }
    println!();

    println!(
        "OpenAI-compatible: {}",
        detected.iter().filter(|p| p.is_openai_compatible()).count()
    );
    println!(
        "Search providers: {}",
        detected
            .iter()
            .filter(|p| matches!(
                p,
                Provider::BraveSearch | Provider::TavilySearch | Provider::SerperSearch
            ))
            .count()
    );
}
