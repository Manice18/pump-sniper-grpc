use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};

#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
    pub creator: [u8; 32],
}

impl BondingCurve {
    pub fn from_account_data(
        data: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if data.len() < 57 {
            return Err("Account data too short".into());
        }
        let offset = 8;
        let virtual_token_reserves = u64::from_le_bytes(data[offset..offset + 8].try_into()?);
        let virtual_sol_reserves = u64::from_le_bytes(data[offset + 8..offset + 16].try_into()?);
        let real_token_reserves = u64::from_le_bytes(data[offset + 16..offset + 24].try_into()?);
        let real_sol_reserves = u64::from_le_bytes(data[offset + 24..offset + 32].try_into()?);
        let token_total_supply = u64::from_le_bytes(data[offset + 32..offset + 40].try_into()?);
        let complete = data[offset + 40] != 0;
        let mut creator = [0u8; 32];
        // Creator immediately follows 'complete' boolean
        creator.copy_from_slice(&data[offset + 41..offset + 73]);
        Ok(BondingCurve {
            virtual_token_reserves,
            virtual_sol_reserves,
            real_token_reserves,
            real_sol_reserves,
            token_total_supply,
            complete,
            creator,
        })
    }
}
