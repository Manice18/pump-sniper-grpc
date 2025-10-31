use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: String,
    pub laserstream_endpoint: String,
    pub helius_rpc_url: String,
    pub slippage_bps: u64,
    pub buy_amount_lamports: u64,
    pub buyer_keypair: String,
    pub min_market_cap_usd: f64,
    pub collection_window_secs: u64,
    pub monitoring_window_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Config {
            api_key: env::var("HELIUS_API_KEY")?,
            laserstream_endpoint: env::var("LASERSTREAM_ENDPOINT")?,
            helius_rpc_url: env::var("HELIUS_ENDPOINT")?,
            slippage_bps: env::var("SLIPPAGE_BPS")
                .unwrap_or_else(|_| "500".to_string())
                .parse()?,
            buy_amount_lamports: env::var("BUY_LAMPORTS")
                .unwrap_or_else(|_| "100000000".to_string())
                .parse()?,
            buyer_keypair: env::var("BUYER_KEYPAIR")?,
            min_market_cap_usd: env::var("MIN_MARKET_CAP_USD")
                .unwrap_or_else(|_| "8000.0".to_string())
                .parse()?,
            collection_window_secs: env::var("COLLECTION_WINDOW_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            monitoring_window_secs: env::var("MONITORING_WINDOW_SECS")
                .unwrap_or_else(|_| "40".to_string())
                .parse()?,
        })
    }

    pub fn min_market_cap_sol(&self, coingecko_sol_usd_price: f64) -> f64 {
        self.min_market_cap_usd / coingecko_sol_usd_price
    }

    pub fn print_info(&self, coingecko_sol_usd_price: f64) {
        println!(
            "🎯 Minimum Market Cap: {:.2} SOL (${:.0})",
            self.min_market_cap_sol(coingecko_sol_usd_price),
            self.min_market_cap_usd
        );
        println!(
            "⏱️  Collection window: {} seconds",
            self.collection_window_secs
        );
        println!(
            "⏱️  Monitoring window: {} seconds",
            self.monitoring_window_secs
        );
        println!("🔍 Monitoring for new tokens...\n");
    }
}
