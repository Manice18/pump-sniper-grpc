use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_GLOBAL: &str = "4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf";
const PUMP_FEE: &str = "CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM";
const PUMP_EVENT_AUTHORITY: &str = "Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const RENT: &str = "SysvarRent111111111111111111111111111111111";

// Buy instruction discriminator
const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

pub struct BuyParams {
    pub mint: String,
    pub bonding_curve: String,
    pub associated_bonding_curve: String,
    pub amount_sol: f64,
    pub slippage_bps: u64, // basis points (e.g., 500 = 5%)
    pub buyer_keypair: Keypair,
}

pub struct BuyTransaction {
    pub transaction: Transaction,
    pub buyer_token_account: String,
    pub estimated_tokens: u64,
}

/// Calculate tokens out with slippage
fn calculate_tokens_with_slippage(
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
    sol_amount: u64,
    slippage_bps: u64,
) -> (u64, u64) {
    let sol_reserves_f64 = virtual_sol_reserves as f64;
    let token_reserves_f64 = virtual_token_reserves as f64;
    let sol_amount_f64 = sol_amount as f64;

    // Calculate expected tokens out
    let tokens_out = (token_reserves_f64 * sol_amount_f64) / (sol_reserves_f64 + sol_amount_f64);

    // Apply slippage
    let slippage_multiplier = 1.0 - (slippage_bps as f64 / 10000.0);
    let min_tokens_out = (tokens_out * slippage_multiplier) as u64;

    (tokens_out as u64, min_tokens_out)
}

/// Build a buy instruction for pump.fun
fn build_buy_instruction(
    buyer: &Pubkey,
    mint: &Pubkey,
    bonding_curve: &Pubkey,
    associated_bonding_curve: &Pubkey,
    buyer_token_account: &Pubkey,
    amount_sol: u64,
    max_sol_cost: u64,
) -> Instruction {
    let global = Pubkey::from_str(PUMP_GLOBAL).unwrap();
    let fee_recipient = Pubkey::from_str(PUMP_FEE).unwrap();
    let event_authority = Pubkey::from_str(PUMP_EVENT_AUTHORITY).unwrap();
    let token_program = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
    let system_program = Pubkey::from_str(SYSTEM_PROGRAM).unwrap();
    let rent = Pubkey::from_str(RENT).unwrap();
    let pump_program = Pubkey::from_str(PUMP_PROGRAM).unwrap();

    let accounts = vec![
        AccountMeta::new(global, false),
        AccountMeta::new(fee_recipient, false),
        AccountMeta::new(*mint, false),
        AccountMeta::new(*bonding_curve, false),
        AccountMeta::new(*associated_bonding_curve, false),
        AccountMeta::new(*buyer_token_account, false),
        AccountMeta::new(*buyer, true), // Signer
        AccountMeta::new_readonly(system_program, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(rent, false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];

    // Build instruction data: discriminator + amount + max_sol_cost
    let mut data = Vec::new();
    data.extend_from_slice(&BUY_DISCRIMINATOR);
    data.extend_from_slice(&amount_sol.to_le_bytes());
    data.extend_from_slice(&max_sol_cost.to_le_bytes());

    Instruction {
        program_id: pump_program,
        accounts,
        data,
    }
}

/// Build a complete buy transaction with compute budget
pub fn build_buy_transaction(
    params: BuyParams,
    rpc_client: &RpcClient,
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
) -> Result<BuyTransaction, Box<dyn std::error::Error>> {
    let buyer = params.buyer_keypair.pubkey();
    let mint = Pubkey::from_str(&params.mint)?;
    let bonding_curve = Pubkey::from_str(&params.bonding_curve)?;
    let associated_bonding_curve = Pubkey::from_str(&params.associated_bonding_curve)?;

    // Get buyer's associated token account
    let buyer_token_account = get_associated_token_address(&buyer, &mint);

    // Convert SOL to lamports
    let amount_lamports = (params.amount_sol * 1_000_000_000.0) as u64;

    // Calculate expected tokens and minimum with slippage
    let (estimated_tokens, min_tokens_out) = calculate_tokens_with_slippage(
        virtual_sol_reserves,
        virtual_token_reserves,
        amount_lamports,
        params.slippage_bps,
    );

    println!("💰 Buy Calculation:");
    println!(
        "   SOL Amount: {} SOL ({} lamports)",
        params.amount_sol, amount_lamports
    );
    println!("   Estimated Tokens Out: {}", estimated_tokens);
    println!(
        "   Min Tokens Out ({}% slippage): {}",
        params.slippage_bps as f64 / 100.0,
        min_tokens_out
    );

    // Build instructions
    let mut instructions = Vec::new();

    // Check if buyer token account exists, if not create ATA instruction
    match rpc_client.get_account(&buyer_token_account) {
        Ok(_) => {
            println!("   ✓ Token account exists: {}", buyer_token_account);
        }
        Err(_) => {
            println!(
                "   ⚠ Creating associated token account: {}",
                buyer_token_account
            );
            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account(
                    &buyer,
                    &buyer,
                    &mint,
                    &Pubkey::from_str(TOKEN_PROGRAM)?,
                );
            instructions.push(create_ata_ix);
        }
    }

    // Add the buy instruction
    let buy_ix = build_buy_instruction(
        &buyer,
        &mint,
        &bonding_curve,
        &associated_bonding_curve,
        &buyer_token_account,
        min_tokens_out,  // Amount of tokens we want (with slippage)
        amount_lamports, // Max SOL we're willing to pay
    );
    instructions.push(buy_ix);

    // Get recent blockhash
    let recent_blockhash = rpc_client.get_latest_blockhash()?;

    // Create message and transaction
    let message = Message::new(&instructions, Some(&buyer));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.sign(&[&params.buyer_keypair], recent_blockhash);

    println!("   ✓ Transaction built successfully");
    println!("   Buyer Token Account: {}", buyer_token_account);

    Ok(BuyTransaction {
        transaction,
        buyer_token_account: buyer_token_account.to_string(),
        estimated_tokens,
    })
}

/// Simulate the transaction without sending it
pub fn simulate_buy_transaction(
    transaction: &Transaction,
    rpc_client: &RpcClient,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Simulating transaction...");

    let config = solana_client::rpc_config::RpcSimulateTransactionConfig {
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    };

    match rpc_client.simulate_transaction_with_config(transaction, config) {
        Ok(response) => {
            if let Some(err) = response.value.err {
                println!("   ❌ Simulation failed: {:?}", err);
                return Err(format!("Simulation error: {:?}", err).into());
            }

            println!("   ✅ Simulation successful!");
            if let Some(logs) = response.value.logs {
                println!("   Logs:");
                for log in logs.iter().take(10) {
                    println!("      {}", log);
                }
            }

            if let Some(units) = response.value.units_consumed {
                println!("   Compute Units: {}", units);
            }
        }
        Err(e) => {
            println!("   ❌ Simulation error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
