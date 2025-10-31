pub fn calculate_market_cap(virtual_sol_reserves: u64, sol_price_usd: f64) -> (f64, f64) {
    let market_cap_sol = virtual_sol_reserves as f64 / 1_000_000_000.0;
    let market_cap_usd = market_cap_sol * sol_price_usd;
    (market_cap_sol, market_cap_usd)
}
