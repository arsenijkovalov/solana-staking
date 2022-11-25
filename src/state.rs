use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::clock::UnixTimestamp;
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct StakingPoolState {
    pub admin: Pubkey,                        // 32 bytes
    pub staking_token_mint: Pubkey,           // 32 bytes
    pub reward_token_mint: Pubkey,            // 32 bytes
    pub total_supply: u64,                    // 8 bytes
    pub reward_per_token_stored: u64,         // 8 bytes
    pub last_update_timestamp: UnixTimestamp, // 8 bytes
}

impl StakingPoolState {
    pub const LEN: usize = 32 * 3 + 8 * 3;

    pub fn unpack(data: &mut [u8]) -> Self {
        StakingPoolState::try_from_slice(data).unwrap()
    }

    pub fn pack(&self, data: &mut [u8]) {
        let encoded = self.try_to_vec().unwrap();
        data[..encoded.len()].copy_from_slice(&encoded);
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct UserState {
    pub balance: u64,               // 8 bytes
    pub reward_per_token_paid: u64, // 8 bytes
    pub rewards: u64,               // 8 bytes
}

impl UserState {
    pub const LEN: usize = 8 * 3;

    pub fn unpack(data: &mut [u8]) -> Self {
        UserState::try_from_slice(data).unwrap()
    }

    pub fn pack(&self, data: &mut [u8]) {
        let encoded = self.try_to_vec().unwrap();
        data[..encoded.len()].copy_from_slice(&encoded);
    }
}
