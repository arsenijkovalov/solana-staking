use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum StakingInstruction {
    ///
    /// 0. [s, w] - authority
    /// 1. [w] - staking pool pda
    /// 2. [] - staking token mint account
    /// 3. [] - rewards token mint account
    /// 4. [w] - staking token escrow pda
    /// 5. [w] - rewards token escrow pda
    /// 6. [] - system program
    /// 7. [] - token program
    /// 8. [] - rent account
    Init,

    ///
    /// 0. [s] - user account who want to stake
    /// 1. [w] - user staking token account
    /// 2. [w] - staking token escrow pda
    /// 3. [w] - user state pda
    /// 4. [w] - staking pool pda
    /// 5. [] - staking token mint account
    /// 6. [] - token program
    /// 7. [] - system program
    Stake { amount: u64 },

    ///
    /// 0. [s] - user account who want to unstake
    /// 1. [w] - user staking token account
    /// 2. [w] - user state pda
    /// 3. [w] - staking pool pda
    /// 4. [w] - staking token escrow pda
    /// 5. [] - staking token escrow pda owner
    /// 6. [] - staking token mint account
    /// 7. [] - token program
    Unstake { amount: u64 },

    ///
    /// 0. [s] - user account who want to claim rewards
    /// 1. [w] - user rewards token account
    /// 2. [w] - user state pda
    /// 3. [w] - staking pool pda
    /// 4. [w] - rewards token escrow pda
    /// 5. [] - staking token escrow pda owner
    /// 6. [] - rewards token mint account
    /// 7. [] - token program
    GetRewards,
}
