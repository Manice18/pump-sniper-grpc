use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use bs58;
use futures_util::StreamExt;
use helius_laserstream::{
    LaserstreamConfig,
    grpc::{CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts},
    subscribe,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use spl_associated_token_account::get_associated_token_address;
use tokio::time::{Duration, sleep};

use crate::execute_ixs::buy;
use crate::utils::config::Config;
use crate::{
    types::{BondingCurve, TokenInfo},
    utils::helper_functions::calculate_market_cap,
};

pub async fn monitor_batches(
    current_batch: Arc<Mutex<Vec<TokenInfo>>>,
    config: Config,
    coingecko_sol_usd_price: f64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        sleep(Duration::from_secs(config.collection_window_secs)).await;

        let batch = {
            let mut current = current_batch.lock().unwrap();
            current.drain(..).collect::<Vec<_>>()
        };

        if batch.is_empty() {
            println!(
                "📦 {}-second window ended. No new tokens collected.\n",
                config.collection_window_secs
            );
            continue;
        }

        println!(
            "\n📦 {}-second collection window ended. Collected {} tokens.",
            config.collection_window_secs,
            batch.len()
        );
        println!(
            "🔍 Starting {}-second monitoring period with Laserstream...\n",
            config.monitoring_window_secs
        );

        if let Err(e) = monitor_batch(batch, &config, coingecko_sol_usd_price).await {
            eprintln!("⚠️ Error monitoring batch: {}", e);
        }
    }
}

async fn monitor_batch(
    batch: Vec<TokenInfo>,
    config: &Config,
    coingecko_sol_usd_price: f64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token_map: HashMap<String, TokenInfo> = batch
        .iter()
        .map(|t| (t.bonding_curve.clone(), t.clone()))
        .collect();

    let bonding_curves: Vec<String> = batch.iter().map(|t| t.bonding_curve.clone()).collect();
    println!("📋 Subscribing to {} bonding curves:", bonding_curves.len());
    for (idx, curve) in bonding_curves.iter().take(10).enumerate() {
        println!("{}: {}", idx + 1, curve);
    }
    if bonding_curves.len() > 10 {
        println!("   ... and {} more", bonding_curves.len() - 10);
    }

    // Build a single subscription entry with all bonding curves
    let accounts_map = HashMap::from([(
        "bonding_curves".to_string(),
        SubscribeRequestFilterAccounts {
            account: bonding_curves,
            owner: vec!["6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string()],
            ..Default::default()
        },
    )]);

    let account_request = SubscribeRequest {
        accounts: accounts_map,
        commitment: Some(CommitmentLevel::Confirmed.into()),
        ..Default::default()
    };

    let laserstream_config =
        LaserstreamConfig::new(config.laserstream_endpoint.clone(), config.api_key.clone());

    println!(
        "🔌 Subscribing to {} bonding curve accounts...",
        batch.len()
    );
    let (account_stream, _account_handle) = subscribe(laserstream_config, account_request);
    tokio::pin!(account_stream);

    let batch_start = std::time::Instant::now();
    let mut found_tokens: HashSet<String> = HashSet::new();

    loop {
        let elapsed = batch_start.elapsed().as_secs();

        if elapsed >= config.monitoring_window_secs {
            println!(
                "⏱️  {}-second monitoring period ended. Checked {} tokens.\n",
                config.monitoring_window_secs,
                batch.len()
            );
            break;
        }

        let timeout_duration = Duration::from_secs(1);

        match tokio::time::timeout(timeout_duration, account_stream.next()).await {
            Ok(Some(Ok(update))) => {
                if let Err(e) = handle_account_update(
                    update,
                    &token_map,
                    &mut found_tokens,
                    elapsed,
                    config,
                    coingecko_sol_usd_price,
                ) {
                    eprintln!("⚠️ Error handling account update: {}", e);
                }
            }
            Ok(Some(Err(e))) => {
                eprintln!("⚠️ Account stream error: {:?}", e);
            }
            Ok(None) => {
                println!("⚠️ Account stream ended unexpectedly");
                break;
            }
            Err(_) => {
                // Timeout - no updates, continue monitoring
            }
        }
    }

    Ok(())
}

fn handle_account_update(
    update: helius_laserstream::grpc::SubscribeUpdate,
    token_map: &HashMap<String, TokenInfo>,
    found_tokens: &mut HashSet<String>,
    elapsed: u64,
    config: &Config,
    coingecko_sol_usd_price: f64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(update_oneof) = &update.update_oneof {
        if let helius_laserstream::grpc::subscribe_update::UpdateOneof::Account(account_update) =
            update_oneof
        {
            if let Some(account) = &account_update.account {
                let account_pubkey = bs58::encode(&account.pubkey).into_string();

                if let Some(token) = token_map.get(&account_pubkey) {
                    // Skip if we've already found this token eligible
                    if found_tokens.contains(&token.mint) {
                        return Ok(());
                    }

                    let curve = BondingCurve::from_account_data(&account.data)?;

                    let market_cap =
                        calculate_market_cap(curve.virtual_sol_reserves, coingecko_sol_usd_price);

                    println!(
                        "📊 Update for {} ({}) at {}s - Market Cap: {:.2} SOL, Market Cap USD: ${:.2}",
                        token.name, token.symbol, elapsed, market_cap.0, market_cap.1
                    );

                    if market_cap.0 >= config.min_market_cap_sol(coingecko_sol_usd_price) {
                        println!(
                            "✅ ELIGIBLE: {} ({}) - Market Cap SOL: {:.2} SOL (${:.0})",
                            token.name, token.symbol, market_cap.0, market_cap.1
                        );
                        println!("   Mint: {}", token.mint);
                        println!("   Bonding Curve: {}", token.bonding_curve);
                        println!("   Creator: {}", token.creator);
                        println!();

                        // Build buy transaction
                        println!("\n🔨 Building buy transaction...");

                        // Calculate associated bonding curve address
                        let mint_pubkey = Pubkey::from_str_const(&token.mint);

                        let bonding_curve_pubkey = Pubkey::from_str_const(&token.bonding_curve);

                        let associated_bonding_curve =
                            get_associated_token_address(&bonding_curve_pubkey, &mint_pubkey);

                        let keypair = Keypair::from_base58_string(&config.buyer_keypair);

                        let buy_params = buy::BuyParams {
                            mint: token.mint.clone(),
                            bonding_curve: token.bonding_curve.clone(),
                            associated_bonding_curve: associated_bonding_curve.to_string(),
                            amount_sol: config.buy_amount_lamports as f64 / 1_000_000_000.0,
                            slippage_bps: config.slippage_bps,
                            buyer_keypair: keypair,
                        };

                        let rpc_client = RpcClient::new(config.helius_rpc_url.clone());

                        match buy::build_buy_transaction(
                            buy_params,
                            &rpc_client,
                            curve.virtual_sol_reserves,
                            curve.virtual_token_reserves,
                        ) {
                            Ok(buy_tx) => {
                                println!("   ✅ Buy transaction built!");
                                println!(
                                    "   📝 Estimated tokens to receive: {}",
                                    buy_tx.estimated_tokens
                                );
                                println!(
                                    "   🏦 Your token account: {}",
                                    buy_tx.buyer_token_account
                                );

                                // Optionally simulate
                                if let Err(e) =
                                    buy::simulate_buy_transaction(&buy_tx.transaction, &rpc_client)
                                {
                                    eprintln!("   ⚠️ Simulation warning: {}", e);
                                }

                                println!("   💾 Transaction ready (not executed)");
                            }
                            Err(e) => {
                                eprintln!("   ❌ Failed to build transaction: {}", e);
                            }
                        }

                        println!();

                        found_tokens.insert(token.mint.clone());
                    }
                }
            }
        }
    }

    Ok(())
}
