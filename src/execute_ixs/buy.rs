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
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const FEE_PROGRAM: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";

// Buy instruction discriminator
const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

pub struct BuyParams {
    pub mint: String,
    pub bonding_curve: String,
    pub associated_bonding_curve: String,
    pub creator: String,
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
    accounts: &BuyAccounts,
    amount_tokens_out: u64,
    max_sol_cost: u64,
) -> Instruction {
    let global = Pubkey::from_str(PUMP_GLOBAL).unwrap();
    let pump_program = Pubkey::from_str(PUMP_PROGRAM).unwrap();

    let metas = vec![
        AccountMeta::new(global, false),
        AccountMeta {
            pubkey: accounts.fee_recipient,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta::new_readonly(accounts.mint, false),
        AccountMeta {
            pubkey: accounts.bonding_curve,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: accounts.associated_bonding_curve,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: accounts.associated_user,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta::new(accounts.user, true),
        AccountMeta::new_readonly(accounts.system_program, false),
        AccountMeta::new_readonly(accounts.token_program, false),
        AccountMeta {
            pubkey: accounts.creator_vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta::new_readonly(accounts.event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
        AccountMeta {
            pubkey: accounts.global_volume_accumulator,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: accounts.user_volume_accumulator,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta::new(accounts.fee_config, false),
        AccountMeta::new_readonly(accounts.fee_program, false),
    ];

    // Build instruction data: discriminator + amount + max_sol_cost + track_volume (OptionBool)
    // OptionBool: Some(bool) = [1, 0] for false, [1, 1] for true, None = [0]
    let mut data = Vec::new();
    data.extend_from_slice(&BUY_DISCRIMINATOR);
    data.extend_from_slice(&amount_tokens_out.to_le_bytes());
    data.extend_from_slice(&max_sol_cost.to_le_bytes());
    // OptionBool::Some(false) = [1, 0]
    data.push(1); // Some
    data.push(0); // false

    Instruction {
        program_id: pump_program,
        accounts: metas,
        data,
    }
}

struct BuyAccounts {
    mint: Pubkey,
    bonding_curve: Pubkey,
    associated_bonding_curve: Pubkey,
    associated_user: Pubkey,
    user: Pubkey,
    system_program: Pubkey,
    token_program: Pubkey,
    creator_vault: Pubkey,
    event_authority: Pubkey,
    global_volume_accumulator: Pubkey,
    user_volume_accumulator: Pubkey,
    fee_config: Pubkey,
    fee_program: Pubkey,
    fee_recipient: Pubkey,
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
    let creator = Pubkey::from_str(&params.creator)?;

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

    // Derive PDAs and required accounts per IDL
    let pump_program = Pubkey::from_str(PUMP_PROGRAM)?;
    let system_program = Pubkey::from_str(SYSTEM_PROGRAM)?;
    let token_program = Pubkey::from_str(TOKEN_PROGRAM)?;
    let fee_program = Pubkey::from_str(FEE_PROGRAM)?;

    // Global PDA
    let (global_pda, _bump) = Pubkey::find_program_address(&[b"global"], &pump_program);
    // Fetch global to read fee_recipient (Anchor: 8-byte discriminator + fields)
    let global_acc = rpc_client.get_account(&global_pda)?;
    let data = global_acc.data;
    // Layout per IDL: bool initialized (1), authority pubkey (32), fee_recipient pubkey (32)
    let fee_recipient_start = 8 + 1 + 32;
    let fee_recipient_end = fee_recipient_start + 32;
    let fee_recipient = Pubkey::new_from_array(
        data[fee_recipient_start..fee_recipient_end]
            .try_into()
            .map_err(|_| "fee_recipient slice error")?,
    );

    // Event authority PDA
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    // Creator vault PDA
    let (creator_vault, _) =
        Pubkey::find_program_address(&[b"creator-vault", &creator.to_bytes()], &pump_program);

    // Global volume accumulator PDA
    let (global_volume_accumulator, _) =
        Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);

    // User volume accumulator PDA
    let (user_volume_accumulator, _) = Pubkey::find_program_address(
        &[b"user_volume_accumulator", &buyer.to_bytes()],
        &pump_program,
    );

    // Fee config PDA: seeds ["fee_config", CONST_32], program = fee_program
    let fee_config_seed2: [u8; 32] = [
        1, 86, 224, 246, 147, 102, 90, 207, 68, 219, 21, 104, 191, 23, 91, 170, 81, 137, 203, 151,
        245, 210, 255, 59, 101, 93, 43, 182, 253, 109, 24, 176,
    ];
    let (fee_config, _) =
        Pubkey::find_program_address(&[b"fee_config", &fee_config_seed2], &fee_program);

    let accounts = BuyAccounts {
        mint,
        bonding_curve,
        associated_bonding_curve,
        associated_user: buyer_token_account,
        user: buyer,
        system_program,
        token_program,
        creator_vault,
        event_authority,
        global_volume_accumulator,
        user_volume_accumulator,
        fee_config,
        fee_program,
        fee_recipient,
    };

    // Add fee buffer (2%) to max_sol_cost to account for protocol fees, creator fees, etc.
    // The program needs ~0.89% more, so 2% should be safe
    let fee_buffer = (amount_lamports as f64 * 0.02) as u64;
    let max_sol_cost_with_fees = amount_lamports + fee_buffer;

    println!("   💰 SOL Budget: {} lamports", amount_lamports);
    println!("   💰 Fee Buffer (2%): {} lamports", fee_buffer);
    println!("   💰 Max SOL Cost: {} lamports", max_sol_cost_with_fees);

    // Add the buy instruction matching IDL order
    let buy_ix = build_buy_instruction(&accounts, min_tokens_out, max_sol_cost_with_fees);
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
                for log in logs.iter() {
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
