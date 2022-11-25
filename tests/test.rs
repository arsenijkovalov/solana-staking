use borsh::BorshDeserialize;
use solana_program::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};
use solana_program_test::{processor, tokio, BanksClient, ProgramTest};
use solana_sdk::{
    program_pack::Pack, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction, transport::TransportError,
};
use spl_token::{
    id,
    state::{Account, Mint},
};
use staking::{
    entrypoint::process_instruction,
    instruction::StakingInstruction,
    state::{StakingPoolState, UserState},
};
use std::str::FromStr;

async fn create_and_initialize_account_for_mint(
    banks_client: &mut BanksClient,
    recent_blockhash: Hash,
    token_program: &Pubkey,
    token_account: &Keypair,
    mint: &Keypair,
    payer: &Keypair,
    owner: &Keypair,
) -> Result<(), ProgramError> {
    let rent = banks_client.get_rent().await.unwrap();
    let account_rent = rent.minimum_balance(Account::LEN);
    let create_account_ix = solana_program::system_instruction::create_account(
        &payer.pubkey(),
        &token_account.pubkey(),
        account_rent,
        Account::LEN as u64,
        token_program,
    );
    let initialize_account_ix = spl_token::instruction::initialize_account(
        token_program,
        &token_account.pubkey(),
        &mint.pubkey(),
        &owner.pubkey(),
    )
    .unwrap();
    let initialize_account_tx = Transaction::new_signed_with_payer(
        &[create_account_ix, initialize_account_ix],
        Some(&payer.pubkey()),
        &[payer, token_account],
        recent_blockhash,
    );
    banks_client
        .process_transaction(initialize_account_tx)
        .await
        .unwrap();
    Ok(())
}

async fn mint_amount(
    banks_client: &mut BanksClient,
    recent_blockhash: Hash,
    token_program: &Pubkey,
    account: &Pubkey,
    mint: &Pubkey,
    mint_authority: &Keypair,
    payer: &Keypair,
    amount: f64,
    mint_decimals: u8,
) -> Result<(), ProgramError> {
    let mint_amount = (amount * f64::powf(10., mint_decimals.into())) as u64;
    let mint_ix = spl_token::instruction::mint_to(
        token_program,
        mint,
        account,
        &mint_authority.pubkey(),
        &[],
        mint_amount,
    )
    .unwrap();
    let mint_tx = Transaction::new_signed_with_payer(
        &[mint_ix],
        Some(&payer.pubkey()),
        &[payer, mint_authority],
        recent_blockhash,
    );
    banks_client.process_transaction(mint_tx).await.unwrap();
    Ok(())
}

async fn get_user_staking_state(banks_client: &mut BanksClient, user_pda: &Pubkey) {
    let user_staking_state_info = banks_client
        .get_account(*user_pda)
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let user_staking_state =
        UserState::try_from_slice(user_staking_state_info.data.as_slice()).unwrap();
    println!();
    println!("------------------ USER STAKING STATE ------------------");
    println!("balance: {}", user_staking_state.balance);
    println!(
        "reward_per_token_paid: {}",
        user_staking_state.reward_per_token_paid
    );
    println!("rewards: {}", user_staking_state.rewards);
    println!("--------------------------------------------------------");
    println!();
}

async fn get_staking_state(banks_client: &mut BanksClient, staking_pool_pda: &Pubkey) {
    let staking_state_info = banks_client
        .get_account(*staking_pool_pda)
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let staking_state =
        StakingPoolState::try_from_slice(staking_state_info.data.as_slice()).unwrap();
    println!();
    println!("------------------ STAKING STATE ------------------");
    println!("admin: {}", staking_state.admin);
    println!("staking_token_mint: {}", staking_state.staking_token_mint);
    println!("reward_token_mint: {}", staking_state.reward_token_mint);
    println!("total_supply: {}", staking_state.total_supply);
    println!(
        "reward_per_token_stored: {}",
        staking_state.reward_per_token_stored
    );
    println!(
        "last_update_timestamp: {}",
        staking_state.last_update_timestamp
    );
    println!("--------------------------------------------------------");
    println!();
}

async fn create_and_initialize_mint(
    banks_client: &mut BanksClient,
    recent_blockhash: Hash,
    payer: &Keypair,
    mint_authority: &Keypair,
    mint_account: &Keypair,
    token_program: &Pubkey,
    decimals: &u8,
) -> Result<(), TransportError> {
    let rent = banks_client.get_rent().await.unwrap();
    let mint_rent = rent.minimum_balance(Mint::LEN);
    let token_mint_a_account_ix = solana_program::system_instruction::create_account(
        &payer.pubkey(),
        &mint_account.pubkey(),
        mint_rent,
        Mint::LEN as u64,
        token_program,
    );
    let token_mint_a_ix = spl_token::instruction::initialize_mint(
        token_program,
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        None,
        *decimals,
    )
    .unwrap();
    let token_mint_a_tx = Transaction::new_signed_with_payer(
        &[token_mint_a_account_ix, token_mint_a_ix],
        Some(&payer.pubkey()),
        &[payer, mint_account],
        recent_blockhash,
    );
    banks_client
        .process_transaction(token_mint_a_tx)
        .await
        .unwrap();
    Ok(())
}

pub fn find_staking_token_pda(
    staking_token_mint_account: &Keypair,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&staking_token_mint_account.pubkey().to_bytes(), b"staking-token"],
        program_id,
    )
}

pub fn find_rewards_token_pda(
    rewards_token_mint_account: &Keypair,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&rewards_token_mint_account.pubkey().to_bytes(), b"rewards-token"],
        program_id,
    )
}

pub fn find_staking_pool_pda(
    staking_token_mint_account: &Keypair,
    rewards_token_mint_account: &Keypair,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &staking_token_mint_account.pubkey().to_bytes(),
            &rewards_token_mint_account.pubkey().to_bytes(),
            b"staking-pool",
        ],
        program_id,
    )
}

pub fn find_user_state_pda(
    staking_pool_pda: &Pubkey,
    user_account: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&staking_pool_pda.to_bytes(), &user_account.to_bytes(), b"user-state"],
        program_id,
    )
}

#[tokio::test]
async fn test_init_staking() {
    let program_id = Pubkey::from_str("3emgBhpukxUExLJ1AnMa5NzDHJYZLNNWqtTccHT4mk2j").unwrap(); // Deploy main program, then put correct program_id before testing

    let program_test = ProgramTest::new("staking", program_id, processor!(process_instruction));

    let mut ctx = program_test.start_with_context().await;
    let mut banks_client = ctx.banks_client.clone();

    let recent_blockhash = ctx.last_blockhash;

    let auth = Keypair::new();
    let alice = Keypair::new();

    ctx.banks_client
        .process_transaction(Transaction::new_signed_with_payer(
            &[
                system_instruction::transfer(&ctx.payer.pubkey(), &auth.pubkey(), 1_000_000_000),
                system_instruction::transfer(&ctx.payer.pubkey(), &alice.pubkey(), 1_000_000_000),
            ],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        ))
        .await
        .unwrap();

    let staking_token_mint_account = Keypair::new();
    let rewards_token_mint_account = Keypair::new();

    let alice_staking_token_account = Keypair::new();
    let alice_rewards_token_account = Keypair::new();

    let (staking_pool_pda, _) = find_staking_pool_pda(&staking_token_mint_account, &rewards_token_mint_account, &program_id);

    let token_program = &id();

    let mint_decimals = 9;
    create_and_initialize_mint(
        &mut banks_client,
        recent_blockhash,
        &auth,
        &auth,
        &staking_token_mint_account,
        token_program,
        &mint_decimals,
    )
    .await
    .unwrap();

    create_and_initialize_mint(
        &mut banks_client,
        recent_blockhash,
        &auth,
        &auth,
        &rewards_token_mint_account,
        token_program,
        &mint_decimals,
    )
    .await
    .unwrap();

    create_and_initialize_account_for_mint(
        &mut banks_client,
        recent_blockhash,
        &spl_token::id(),
        &alice_staking_token_account,
        &staking_token_mint_account,
        &auth,
        &alice,
    )
    .await
    .unwrap();

    create_and_initialize_account_for_mint(
        &mut banks_client,
        recent_blockhash,
        &spl_token::id(),
        &alice_rewards_token_account,
        &rewards_token_mint_account,
        &auth,
        &alice,
    )
    .await
    .unwrap();

    let amount = 1000.0;
    mint_amount(
        &mut banks_client,
        recent_blockhash,
        &spl_token::id(),
        &alice_staking_token_account.pubkey(),
        &staking_token_mint_account.pubkey(),
        &auth,
        &auth,
        amount,
        mint_decimals,
    )
    .await
    .unwrap();

    let minted_amount = (amount * f64::powf(10., mint_decimals.into())) as u64;

    let token_account_info = banks_client
        .get_account(alice_staking_token_account.pubkey().clone())
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let account_data = Account::unpack(&token_account_info.data).unwrap();
    assert_eq!(
        account_data.amount, minted_amount,
        "Initial minting tokens failed"
    );

    /*-------------------- INITIALIZE ---------------------*/
    
    let (staking_token_escrow_pda, _) = find_staking_token_pda(&staking_token_mint_account, &program_id);
    let (rewards_token_escrow_pda, _) = find_rewards_token_pda(&rewards_token_mint_account, &program_id);

    let init_accounts = vec![
        AccountMeta::new(auth.pubkey(), true),
        AccountMeta::new(staking_pool_pda, false),
        AccountMeta::new_readonly(staking_token_mint_account.pubkey(), false),
        AccountMeta::new_readonly(rewards_token_mint_account.pubkey(), false),
        AccountMeta::new(staking_token_escrow_pda, false),
        AccountMeta::new(rewards_token_escrow_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program, false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];
    let init_ix = Instruction::new_with_borsh(program_id, &StakingInstruction::Init, init_accounts);
    let init_tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&auth.pubkey()),
        &[&auth],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(init_tx).await.unwrap();

    mint_amount(
        &mut banks_client,
        recent_blockhash,
        &spl_token::id(),
        &rewards_token_escrow_pda,
        &rewards_token_mint_account.pubkey(),
        &auth,
        &auth,
        amount,
        mint_decimals,
    )
    .await
    .unwrap();

    println!("Staking state after initialization");
    get_staking_state(&mut banks_client, &staking_pool_pda).await;

    /*-----------------------------------------------------*/

    /*----------------------- STAKE -----------------------*/

    let (user_state_pda, _) =
        find_user_state_pda(&staking_pool_pda, &alice.pubkey(), &program_id);

    let stake_accounts = vec![
        AccountMeta::new_readonly(alice.pubkey(), true),
        AccountMeta::new(alice_staking_token_account.pubkey(), false),
        AccountMeta::new(staking_token_escrow_pda, false),
        AccountMeta::new(user_state_pda, false),
        AccountMeta::new(staking_pool_pda, false),
        AccountMeta::new_readonly(staking_token_mint_account.pubkey(), false),
        AccountMeta::new_readonly(*token_program, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    let stake_amount = 100;
    let stake_ix = Instruction::new_with_borsh(
        program_id,
        &StakingInstruction::Stake {
            amount: stake_amount,
        },
        stake_accounts,
    );
    let stake_tx = Transaction::new_signed_with_payer(
        &[stake_ix],
        Some(&alice.pubkey()),
        &[&alice],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction(stake_tx)
        .await
        .unwrap();

    println!("Alice stakes {} tokens", stake_amount);
    get_user_staking_state(&mut banks_client, &user_state_pda).await;

    println!("Staking state after Alice's stake");
    get_staking_state(&mut banks_client, &staking_pool_pda).await;

    let token_account_info = banks_client
        .get_account(alice_staking_token_account.pubkey().clone())
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let account_data = Account::unpack(&token_account_info.data).unwrap();
    assert_eq!(
        account_data.amount,
        minted_amount - stake_amount,
        "Stake operation was incorrect"
    );

    /*-----------------------------------------------------*/

    /*----------------------- UNSTAKE ---------------------*/

    let (staking_token_escrow_pda_owner, _) = find_staking_token_pda(&staking_token_mint_account, &program_id);

    let unstake_accounts = vec![
        AccountMeta::new_readonly(alice.pubkey(), true),
        AccountMeta::new(alice_staking_token_account.pubkey(), false),
        AccountMeta::new(user_state_pda, false),
        AccountMeta::new(staking_pool_pda, false),
        AccountMeta::new(staking_token_escrow_pda, false),
        AccountMeta::new_readonly(staking_token_escrow_pda_owner, false),
        AccountMeta::new_readonly(staking_token_mint_account.pubkey(), false),
        AccountMeta::new_readonly(*token_program, false),
    ];
    let unstake_amount = 50;
    let unstake_ix = Instruction::new_with_borsh(
        program_id,
        &StakingInstruction::Unstake {
            amount: unstake_amount,
        },
        unstake_accounts,
    );
    let unstake_tx = Transaction::new_signed_with_payer(
        &[unstake_ix],
        Some(&alice.pubkey()),
        &[&alice],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction(unstake_tx)
        .await
        .unwrap();

    println!("Alice withdraws {} tokens", unstake_amount);
    get_user_staking_state(&mut banks_client, &user_state_pda).await;

    println!("Staking state after Alice's withdraw");
    get_staking_state(&mut banks_client, &staking_pool_pda).await;

    let token_account_info = banks_client
        .get_account(alice_staking_token_account.pubkey().clone())
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let account_data = Account::unpack(&token_account_info.data).unwrap();
    assert_eq!(
        account_data.amount,
        minted_amount - stake_amount + unstake_amount,
        "Unstake operation was incorrect"
    );

    /*----------------------------------------------------*/

    /*-------------------- GET REWARDS -------------------*/

    let token_account_info = banks_client
        .get_account(alice_rewards_token_account.pubkey().clone())
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let account_data = Account::unpack(&token_account_info.data).unwrap();
    println!("Alice rewards token account balance before withdrawing rewards {}", account_data.amount);

    let (rewards_token_escrow_pda_owner, _) = find_rewards_token_pda(&rewards_token_mint_account, &program_id);

    let rewards_accounts = vec![
        AccountMeta::new_readonly(alice.pubkey(), true),
        AccountMeta::new(alice_rewards_token_account.pubkey(), false),
        AccountMeta::new(user_state_pda, false),
        AccountMeta::new(staking_pool_pda, false),
        AccountMeta::new(rewards_token_escrow_pda, false),
        AccountMeta::new_readonly(rewards_token_escrow_pda_owner, false),
        AccountMeta::new_readonly(rewards_token_mint_account.pubkey(), false),
        AccountMeta::new_readonly(*token_program, false),
    ];
    let rewards_ix = Instruction::new_with_borsh(
        program_id,
        &StakingInstruction::GetRewards,
        rewards_accounts,
    );
    let rewards_tx = Transaction::new_signed_with_payer(
        &[rewards_ix],
        Some(&alice.pubkey()),
        &[&alice],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction(rewards_tx)
        .await
        .unwrap();

    println!("Alice gets rewards");
    get_user_staking_state(&mut banks_client, &user_state_pda).await;

    println!("Staking state after Alice gets rewards");
    get_staking_state(&mut banks_client, &staking_pool_pda).await;

    let token_account_info = banks_client
        .get_account(alice_rewards_token_account.pubkey().clone())
        .await
        .unwrap()
        .expect("Could not fetch account information");
    let account_data = Account::unpack(&token_account_info.data).unwrap();
    println!("Alice rewards token account balance after withdrawing rewards {}", account_data.amount);
}
