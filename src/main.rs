use std::collections::HashSet;
use std::env;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

use serde_json::Value;

use monitors::{monitor_account, monitor_transaction};
use types::TokenInfo;
use utils::config::Config;

mod execute_ixs;
mod monitors;
mod parser;
mod types;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv::from_path(".env").ok();

    let coingecko_endpoint = env::var("COINGECKO_URL").expect("COINGECKO_URL must be set");

    let coingecko_resp = reqwest::get(coingecko_endpoint).await?.text().await?;
    let coingecko_data: Value = serde_json::from_str(&coingecko_resp)?;
    let coingecko_sol_usd_price = coingecko_data["solana"]["usd"].as_f64().unwrap_or(0.0);

    let config = Config::from_env()?;
    config.print_info(coingecko_sol_usd_price);

    let current_batch: Arc<Mutex<Vec<TokenInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let processed_tokens: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    println!("🔍 Starting account monitoring...");
    // Spawn account monitoring task
    let account_monitor = tokio::spawn(monitor_account::monitor_batches(
        current_batch.clone(),
        config.clone(),
        coingecko_sol_usd_price,
    ));

    // Start transaction monitoring (blocks on main thread)
    monitor_transaction::monitor_transactions(current_batch, processed_tokens, config).await?;

    account_monitor.await??;
    Ok(())
}
