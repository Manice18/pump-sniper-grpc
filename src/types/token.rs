#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub mint: String,
    pub bonding_curve: String,
    pub name: String,
    pub symbol: String,
    pub creator: String,
    pub created_at: std::time::Instant,
}

impl TokenInfo {
    pub fn new(
        mint: String,
        bonding_curve: String,
        name: String,
        symbol: String,
        creator: String,
    ) -> Self {
        Self {
            mint,
            bonding_curve,
            name,
            symbol,
            creator,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn print_creation(&self) {
        println!("🆕 NEW TOKEN CREATED!");
        println!("   Name: {}", self.name);
        println!("   Symbol: {}", self.symbol);
        println!("   Mint: {}", self.mint);
        println!("   Bonding Curve: {}", self.bonding_curve);
        println!("   Creator: {}", self.creator);
        println!("   📦 Added to current collection batch");
        println!();
    }
}
