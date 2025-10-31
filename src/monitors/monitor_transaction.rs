use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use bs58;
use futures_util::StreamExt;
use helius_laserstream::{
    LaserstreamConfig,
    grpc::{CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions},
    subscribe,
};

use crate::parser::parse_create_instruction;
use crate::types::TokenInfo;
use crate::utils::config::Config;
use crate::utils::constants::{CREATE_DISCRIMINATOR, PUMP_PROGRAM};

pub async fn monitor_transactions(
    current_batch: Arc<Mutex<Vec<TokenInfo>>>,
    processed_tokens: Arc<Mutex<HashSet<String>>>,
    config: Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let laserstream_config = LaserstreamConfig::new(config.laserstream_endpoint, config.api_key);

    let request = SubscribeRequest {
        transactions: HashMap::from([(
            "pump-txs".to_string(),
            SubscribeRequestFilterTransactions {
                account_include: vec![PUMP_PROGRAM.to_string()],
                vote: Some(false),
                failed: Some(false),
                ..Default::default()
            },
        )]),
        commitment: Some(CommitmentLevel::Confirmed.into()),
        ..Default::default()
    };

    println!("🔌 Connecting to transaction stream...");
    let (stream, _handle) = subscribe(laserstream_config, request);
    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(update) => {
                if let Some(txs) = &update.update_oneof {
                    if let helius_laserstream::grpc::subscribe_update::UpdateOneof::Transaction(
                        tx,
                    ) = txs
                    {
                        if let Some(info) = &tx.transaction {
                            if let Some(transaction) = &info.transaction {
                                if let Some(message) = &transaction.message {
                                    for ix in &message.instructions {
                                        if ix.data.starts_with(&CREATE_DISCRIMINATOR) {
                                            if let Err(e) = handle_create_instruction(
                                                &ix.data,
                                                &message.account_keys,
                                                &current_batch,
                                                &processed_tokens,
                                            ) {
                                                eprintln!(
                                                    "⚠️ Failed to handle CREATE instruction: {}",
                                                    e
                                                );
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("⚠️ Transaction Stream Error: {:?}", e),
        }
    }

    Ok(())
}

fn handle_create_instruction(
    data: &[u8],
    account_keys: &[Vec<u8>],
    current_batch: &Arc<Mutex<Vec<TokenInfo>>>,
    processed_tokens: &Arc<Mutex<HashSet<String>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (name, symbol) = parse_create_instruction(data)?;

    if account_keys.len() < 3 {
        return Err("Not enough account keys".into());
    }

    let mint = bs58::encode(&account_keys[1]).into_string();
    let bonding_curve = bs58::encode(&account_keys[2]).into_string();
    let creator = bs58::encode(&account_keys[0]).into_string();

    // Check if already processed
    let mut processed = processed_tokens.lock().unwrap();
    if processed.contains(&mint) {
        return Ok(());
    }
    processed.insert(mint.clone());
    drop(processed);

    let token_info = TokenInfo::new(mint, bonding_curve, name, symbol, creator);
    token_info.print_creation();

    let mut batch = current_batch.lock().unwrap();
    batch.push(token_info);

    Ok(())
}
