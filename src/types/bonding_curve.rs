use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};

#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
}

impl BondingCurve {
    pub fn from_account_data(
        data: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if data.len() < 57 {
            return Err("Account data too short".into());
        }
        let offset = 8;
        Ok(BondingCurve {
            virtual_token_reserves: u64::from_le_bytes(data[offset..offset + 8].try_into()?),
            virtual_sol_reserves: u64::from_le_bytes(data[offset + 8..offset + 16].try_into()?),
            real_token_reserves: u64::from_le_bytes(data[offset + 16..offset + 24].try_into()?),
            real_sol_reserves: u64::from_le_bytes(data[offset + 24..offset + 32].try_into()?),
            token_total_supply: u64::from_le_bytes(data[offset + 32..offset + 40].try_into()?),
            complete: data[offset + 40] != 0,
        })
    }
}
