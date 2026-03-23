//! Model/Key Mux CLI Menu - Top-bar provider selection app
//!
//! Usage: mux-menu [COMMAND]
//!   discover  - Show available providers
//!   route     - Route a model to a provider
//!   status    - Show provider quotas

use ccek_keymux::{MuxMenu, discover_providers};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    let mut menu = MuxMenu::new();

    match command {
        "discover" => {
            println!("=== Active Providers ===");
            for name in menu.provider_names() {
                println!("  • {}", name);
            }
        }
        "route" => {
            if let Some(model_ref) = args.get(2) {
                if let Some((provider, model, api_key)) = menu.route(model_ref) {
                    println!("Provider: {}", provider);
                    println!("Model: {}", model);
                    println!("API Key: {}...", &api_key[..8.min(api_key.len())]);
                } else {
                    eprintln!("No route found for: {}", model_ref);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Usage: mux-menu route <model-ref>");
                std::process::exit(1);
            }
        }
        "select" => {
            if let Some(provider) = args.get(2) {
                if menu.select_provider(provider) {
                    println!("Selected provider: {}", provider);
                } else {
                    eprintln!("Provider not found: {}", provider);
                    std::process::exit(1);
                }
            } else {
                eprintln!("Usage: mux-menu select <provider>");
                std::process::exit(1);
            }
        }
        "status" | _ => {
            println!("=== Model/Key Mux Status ===");
            println!();
            println!("Active Providers:");
            for name in menu.provider_names() {
                println!("  • {}", name);
            }
            println!();
            
            if let Some(ref selected) = menu.selected_provider {
                println!("Selected: {}", selected);
            } else {
                println!("Selected: (none)");
            }
            
            println!();
            println!("Quotas:");
            for (name, quota) in &menu.quotas {
                println!("  {}: {}/{} tokens ({:.1}% confidence)",
                    name,
                    quota.used_tokens,
                    quota.used_tokens + quota.remaining_tokens,
                    quota.confidence * 100.0
                );
            }
            
            println!();
            println!("Commands: discover, route <model>, select <provider>, status");
        }
    }
}
