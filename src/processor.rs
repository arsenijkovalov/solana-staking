use crate::instruction::StakingInstruction;
use crate::pda_helper::PdaHelper;
use crate::state::StakingPoolState;
use crate::state::UserState;
use borsh::BorshDeserialize;
use solana_program::clock::UnixTimestamp;
use solana_program::program::invoke;
use solana_program::program::invoke_signed;
use solana_program::sysvar::clock::Clock;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::next_account_info, account_info::AccountInfo, entrypoint::ProgramResult, msg,
    program_error::ProgramError, program_pack::Pack, pubkey::Pubkey, rent::Rent,
};
use spl_token::state::Account;

pub struct Processor;

impl Processor {
    const REWARD_RATE: u64 = 100;

    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instructions = StakingInstruction::try_from_slice(instruction_data)?;
        match instructions {
            StakingInstruction::Init => Self::initialize(program_id, accounts),
            StakingInstruction::Stake { amount } => Self::stake(program_id, accounts, amount),
            StakingInstruction::Unstake { amount } => Self::unstake(program_id, accounts, amount),
            StakingInstruction::GetRewards => Self::get_rewards(program_id, accounts),
        }
    }

    fn initialize(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();
        let authority = next_account_info(accounts_iter)?;
        let staking_pool_pda_ai = next_account_info(accounts_iter)?;
        let staking_token_mint_account = next_account_info(accounts_iter)?;
        let rewards_token_mint_account = next_account_info(accounts_iter)?;
        let staking_token_escrow_pda = next_account_info(accounts_iter)?;
        let rewards_token_escrow_pda = next_account_info(accounts_iter)?;
        let system_program = next_account_info(accounts_iter)?;
        let token_program = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;
        let clock = Clock::get()?;
        let (staking_pool_pda, bump_seed) = PdaHelper::find_staking_pool_pda(
            staking_token_mint_account,
            rewards_token_mint_account,
            program_id,
        );
        if *staking_pool_pda_ai.key != staking_pool_pda {
            msg!("Staking pool pda passed: {}", staking_pool_pda_ai.key);
            msg!("Staking pool pda computed: {}", staking_pool_pda);
            return Err(ProgramError::InvalidAccountData);
        }
        if !staking_pool_pda_ai.try_data_is_empty()? {
            let mut staking_state =
                StakingPoolState::try_from_slice(&staking_pool_pda_ai.try_borrow_mut_data()?)?;
            staking_state.admin = *authority.key;
            staking_state.staking_token_mint = *staking_token_mint_account.key;
            staking_state.reward_token_mint = *rewards_token_mint_account.key;
            staking_state.total_supply = 0;
            staking_state.last_update_timestamp = clock.unix_timestamp;
            staking_state.reward_per_token_stored = 0;
            staking_state.pack(&mut staking_pool_pda_ai.try_borrow_mut_data()?);
            msg!("Reset staking pool values");
        } else {
            msg!("Trying to create account");
            let create_account_ix = solana_program::system_instruction::create_account(
                &authority.key,
                &staking_pool_pda,
                Rent::get()?.minimum_balance(StakingPoolState::LEN),
                StakingPoolState::LEN as u64,
                program_id,
            );
            invoke_signed(
                &create_account_ix,
                &[
                    authority.clone(),
                    staking_pool_pda_ai.clone(),
                    system_program.clone(),
                ],
                &[&[
                    staking_token_mint_account.key.as_ref(),
                    rewards_token_mint_account.key.as_ref(),
                    b"staking-pool",
                    &[bump_seed],
                ]],
            )?;
            msg!("Staking pool pda created: {}", staking_pool_pda_ai.key);
            let mut staking_state =
                StakingPoolState::try_from_slice(&staking_pool_pda_ai.try_borrow_mut_data()?)?;
            staking_state.admin = *authority.key;
            staking_state.staking_token_mint = *staking_token_mint_account.key;
            staking_state.reward_token_mint = *rewards_token_mint_account.key;
            staking_state.total_supply = 0;
            staking_state.last_update_timestamp = clock.unix_timestamp;
            staking_state.reward_per_token_stored = 0;
            staking_state.pack(&mut staking_pool_pda_ai.try_borrow_mut_data()?);
            msg!("Initialized staking with next values: ");
            msg!("admin: {}", authority.key);
            msg!(
                "staking token mint pubkey: {}",
                staking_token_mint_account.key
            );
            msg!(
                "reward token mint pubkey: {}",
                rewards_token_mint_account.key
            );
            msg!("total staked: {}", 0);
            msg!("last reward timestamp: {}", clock.unix_timestamp);
            let (staking_token_pda, staking_token_bump_seed) =
                PdaHelper::find_staking_token_pda(staking_token_mint_account, program_id);
            invoke_signed(
                &solana_program::system_instruction::create_account(
                    &authority.key,
                    &staking_token_pda,
                    Rent::get()?.minimum_balance(spl_token::state::Account::LEN),
                    spl_token::state::Account::LEN as u64,
                    &token_program.key,
                ),
                &[
                    authority.clone(),
                    staking_token_escrow_pda.clone(),
                    token_program.clone(),
                ],
                &[&[
                    staking_token_mint_account.key.as_ref(),
                    b"staking-token",
                    &[staking_token_bump_seed],
                ]],
            )?;
            let ix = spl_token::instruction::initialize_account(
                &token_program.key,
                &staking_token_escrow_pda.key,
                &staking_token_mint_account.key,
                &staking_token_pda,
            )?;
            invoke(
                &ix,
                &[
                    staking_token_escrow_pda.clone(),
                    staking_token_mint_account.clone(),
                    staking_pool_pda_ai.clone(),
                    rent_account.clone(),
                    token_program.clone(),
                ],
            )?;
            let (rewards_token_pda, rewards_token_bump_seed) =
                PdaHelper::find_rewards_token_pda(rewards_token_mint_account, program_id);
            invoke_signed(
                &solana_program::system_instruction::create_account(
                    &authority.key,
                    &rewards_token_pda,
                    Rent::get()?.minimum_balance(spl_token::state::Account::LEN),
                    spl_token::state::Account::LEN as u64,
                    &token_program.key,
                ),
                &[
                    authority.clone(),
                    rewards_token_escrow_pda.clone(),
                    token_program.clone(),
                ],
                &[&[
                    rewards_token_mint_account.key.as_ref(),
                    b"rewards-token",
                    &[rewards_token_bump_seed],
                ]],
            )?;
            let ix = spl_token::instruction::initialize_account(
                &token_program.key,
                &rewards_token_escrow_pda.key,
                &rewards_token_mint_account.key,
                &rewards_token_pda,
            )?;
            invoke(
                &ix,
                &[
                    rewards_token_escrow_pda.clone(),
                    rewards_token_mint_account.clone(),
                    staking_pool_pda_ai.clone(),
                    rent_account.clone(),
                    token_program.clone(),
                ],
            )?;
        };
        let (staking_token_pda, _) =
            PdaHelper::find_staking_token_pda(staking_token_mint_account, program_id);
        let (rewards_token_pda, _) =
            PdaHelper::find_rewards_token_pda(rewards_token_mint_account, program_id);
        let staking_token_account = Account::unpack(&staking_token_escrow_pda.try_borrow_data()?)?;
        let rewards_token_account = Account::unpack(&rewards_token_escrow_pda.try_borrow_data()?)?;
        if staking_token_account.owner != staking_token_pda {
            msg!(
                "Stake token account must have pda as owner. Current owner {}, pda {}",
                staking_token_account.owner,
                staking_token_pda
            );
            return Err(ProgramError::InvalidAccountData);
        }
        if rewards_token_account.owner != rewards_token_pda {
            msg!(
                "Rewards token account must have pda as owner. Current owner {}, pda {}",
                rewards_token_account.owner,
                rewards_token_pda
            );
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    fn stake(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();
        let user_authority = next_account_info(accounts_iter)?;
        let staking_token_account = next_account_info(accounts_iter)?;
        let staking_token_escrow_pda = next_account_info(accounts_iter)?;
        let user_state_pda_ai = next_account_info(accounts_iter)?;
        let staking_pool_pda = next_account_info(accounts_iter)?;
        let staking_token_mint_account = next_account_info(accounts_iter)?;
        let token_program = next_account_info(accounts_iter)?;
        let system_program = next_account_info(accounts_iter)?;
        if amount == 0 {
            msg!("Amount = 0");
            return Err(ProgramError::InvalidInstructionData);
        }
        let (staking_token_escrow_pda_owner, _) =
            PdaHelper::find_staking_token_pda(staking_token_mint_account, program_id);
        let staking_token_escrow_account =
            Account::unpack(&staking_token_escrow_pda.try_borrow_data()?)?;
        if staking_token_escrow_account.owner != staking_token_escrow_pda_owner {
            msg!(
                "Stake token account must have pda as owner. Current owner {}, pda {}",
                staking_token_escrow_account.owner,
                staking_token_escrow_pda_owner
            );
            return Err(ProgramError::InvalidAccountData);
        }
        let transafer_ix = spl_token::instruction::transfer(
            &token_program.key,
            &staking_token_account.key,
            &staking_token_escrow_pda.key,
            &user_authority.key,
            &[],
            amount,
        )?;
        invoke(
            &transafer_ix,
            &[
                staking_token_account.clone(),
                staking_token_escrow_pda.clone(),
                user_authority.clone(),
                token_program.clone(),
            ],
        )?;
        msg!(
            "Tokens transfered from staker {} to pda {}",
            staking_token_account.key,
            staking_token_escrow_pda.key
        );
        let (user_state_pda, bump_seed) =
            PdaHelper::find_user_state_pda(staking_pool_pda, user_authority, program_id);
        msg!("Staker pda: {}", user_state_pda_ai.key);
        msg!("Staker pda computed: {}", user_state_pda);
        if user_state_pda_ai.try_data_is_empty()? {
            let create_acc_ix = solana_program::system_instruction::create_account(
                &user_authority.key,
                &user_state_pda,
                Rent::get()?.minimum_balance(UserState::LEN),
                UserState::LEN as u64,
                &program_id,
            );
            invoke_signed(
                &create_acc_ix,
                &[
                    user_authority.clone(),
                    user_state_pda_ai.clone(),
                    system_program.clone(),
                ],
                &[&[
                    &staking_pool_pda.key.to_bytes(),
                    &user_authority.key.to_bytes(),
                    b"user-state",
                    &[bump_seed],
                ]],
            )?;
        }
        Self::update_rewards(staking_pool_pda, user_state_pda_ai)?;
        let mut user_state = UserState::try_from_slice(&user_state_pda_ai.try_borrow_data()?)?;
        user_state.balance += amount;
        user_state.pack(&mut user_state_pda_ai.try_borrow_mut_data()?);
        let mut staking_pool_state =
            StakingPoolState::unpack(&mut staking_pool_pda.try_borrow_mut_data()?);
        staking_pool_state.total_supply += amount;
        staking_pool_state.pack(&mut staking_pool_pda.try_borrow_mut_data()?);
        msg!("STAKE From: {} Amount: {}", user_authority.key, amount);
        Ok(())
    }

    fn unstake(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();
        let user_authority = next_account_info(accounts_iter)?;
        let staking_token_account = next_account_info(accounts_iter)?; // user_staking_token_ai
        let user_state_pda = next_account_info(accounts_iter)?;
        let staking_pool_pda = next_account_info(accounts_iter)?;
        let staking_token_escrow_pda = next_account_info(accounts_iter)?;
        let staking_token_escrow_pda_owner_ai = next_account_info(accounts_iter)?;
        let staking_token_mint_account = next_account_info(accounts_iter)?;
        let token_program = next_account_info(accounts_iter)?;
        if !user_authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if amount == 0 {
            msg!("Amount = 0");
            return Err(ProgramError::InvalidInstructionData);
        }
        Self::update_rewards(staking_pool_pda, user_state_pda)?;
        let users_state = UserState::unpack(&mut user_state_pda.try_borrow_mut_data()?);
        if amount > users_state.balance {
            msg!(
                "Cannot unstake more than staked. Staked: {}, trying to withdraw: {}",
                users_state.balance,
                amount
            );
            return Err(ProgramError::InvalidInstructionData);
        }
        let (staking_token_escrow_pda_owner, bump) =
            PdaHelper::find_staking_token_pda(staking_token_mint_account, program_id);
        let staking_token_escrow_account =
            Account::unpack_from_slice(&mut staking_token_escrow_pda.try_borrow_mut_data()?)?;
        if staking_token_escrow_account.owner != staking_token_escrow_pda_owner {
            msg!(
                "Passed escrow staking owner: {}",
                staking_token_escrow_pda_owner_ai.key
            );
            msg!(
                "Computed escrow staking owner: {}",
                staking_token_escrow_pda_owner
            );
            return Err(ProgramError::InvalidAccountData);
        }
        let transfer_ix = spl_token::instruction::transfer(
            token_program.key,
            staking_token_escrow_pda.key,
            staking_token_account.key,
            &staking_token_escrow_pda_owner,
            &[],
            amount,
        )?;
        invoke_signed(
            &transfer_ix,
            &[
                staking_token_escrow_pda.clone(),
                staking_token_account.clone(),
                staking_token_escrow_pda_owner_ai.clone(),
                token_program.clone(),
            ],
            &[&[
                &staking_token_mint_account.key.to_bytes(),
                b"staking-token",
                &[bump],
            ]],
        )?;
        let mut user_state = UserState::unpack(&mut user_state_pda.try_borrow_mut_data()?);
        user_state.balance -= amount;
        user_state.pack(&mut user_state_pda.try_borrow_mut_data()?);
        let mut staking_pool_state =
            StakingPoolState::unpack(&mut staking_pool_pda.try_borrow_mut_data()?);
        staking_pool_state.total_supply -= amount;
        staking_pool_state.pack(&mut staking_pool_pda.try_borrow_mut_data()?);
        msg!(
            "UNSTAKE Transfer: {} From: {} To: {}",
            amount,
            staking_token_escrow_pda.key,
            staking_token_account.key
        );
        Ok(())
    }

    fn get_rewards(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();
        let user_authority = next_account_info(accounts_iter)?;
        let rewards_token_account = next_account_info(accounts_iter)?;
        let user_state_pda = next_account_info(accounts_iter)?;
        let staking_pool_pda = next_account_info(accounts_iter)?;
        let rewards_token_escrow_pda = next_account_info(accounts_iter)?;
        let rewards_token_escrow_pda_owner_ai = next_account_info(accounts_iter)?;
        let rewards_token_mint_account = next_account_info(accounts_iter)?;
        let token_program = next_account_info(accounts_iter)?;
        if !user_authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Self::update_rewards(staking_pool_pda, user_state_pda)?;
        let user_rewards = Self::get_user_rewards(user_state_pda);
        if user_rewards > 0 {
            let (rewards_token_escrow_pda_owner, bump_seed) =
                PdaHelper::find_rewards_token_pda(rewards_token_mint_account, program_id);
            let transfer_ix = spl_token::instruction::transfer(
                token_program.key,
                rewards_token_escrow_pda.key,
                rewards_token_account.key,
                &rewards_token_escrow_pda_owner,
                &[],
                user_rewards,
            )?;
            invoke_signed(
                &transfer_ix,
                &[
                    rewards_token_escrow_pda.clone(),
                    rewards_token_account.clone(),
                    rewards_token_escrow_pda_owner_ai.clone(),
                    token_program.clone(),
                ],
                &[&[
                    &rewards_token_mint_account.key.to_bytes(),
                    b"rewards-token",
                    &[bump_seed],
                ]],
            )?;
            let mut user_state = UserState::unpack(&mut user_state_pda.try_borrow_mut_data()?);
            user_state.rewards = 0;
            user_state.pack(&mut user_state_pda.try_borrow_mut_data()?);
        }
        Ok(())
    }

    fn update_rewards(
        staking_pool_pda: &AccountInfo,
        user_state_pda: &AccountInfo,
    ) -> ProgramResult {
        let last_update_timestamp = Clock::get().unwrap().unix_timestamp;
        let mut staking_pool_state =
            StakingPoolState::unpack(&mut staking_pool_pda.try_borrow_mut_data().unwrap());
        let mut user_state = UserState::unpack(&mut user_state_pda.try_borrow_mut_data().unwrap());
        let rewards_per_token_stored = Self::reward_per_token(&staking_pool_state);
        staking_pool_state.reward_per_token_stored = rewards_per_token_stored;
        staking_pool_state.last_update_timestamp = last_update_timestamp;
        user_state.rewards = Self::earned(&staking_pool_state, &user_state);
        user_state.reward_per_token_paid = rewards_per_token_stored;
        staking_pool_state.pack(&mut staking_pool_pda.try_borrow_mut_data()?);
        user_state.pack(&mut user_state_pda.try_borrow_mut_data()?);
        Ok(())
    }

    fn get_user_rewards(user_state_pda: &AccountInfo) -> u64 {
        let user_state = UserState::unpack(&mut user_state_pda.try_borrow_mut_data().unwrap());
        return user_state.rewards;
    }

    fn reward_per_token(staking_pool_state: &StakingPoolState) -> u64 {
        let total_supply = staking_pool_state.total_supply;
        let reward_per_token_stored = staking_pool_state.reward_per_token_stored;
        if total_supply == 0 {
            return reward_per_token_stored;
        }
        let last_update_timestamp = staking_pool_state.last_update_timestamp;
        let current_timestamp = Clock::get().unwrap().unix_timestamp + 100; // For testing purposes only
        reward_per_token_stored
            + (Self::REWARD_RATE
                * ((current_timestamp - last_update_timestamp) as u64)
                * 10_000_000_000)
                / total_supply
    }

    fn earned(staking_pool_state: &StakingPoolState, user_state: &UserState) -> u64 {
        let stake_amount = user_state.balance;
        let user_reward_per_token_paid = user_state.reward_per_token_paid;
        let user_rewards = user_state.rewards;
        let reward_per_token = Self::reward_per_token(&staking_pool_state);
        (stake_amount * (reward_per_token - user_reward_per_token_paid) / 10_000_000_000)
            + user_rewards
    }
}
