use {
    amm_video::{error::AmmError, state::Config},
    anchor_lang::AccountDeserialize,
    anchor_spl::{
        associated_token,
        token::{Mint, TokenAccount},
    },
    litesvm::{types::TransactionResult, LiteSVM},
    litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo},
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

const SEED: u64 = 123;
const FEE: u16 = 30;
const PROTOCOL_FEE: u16 = 20;
const USER_FUNDS: u64 = 1_000_000_000;

const INITIAL_LP: u64 = 100_000_000;
const INITIAL_X: u64 = 200_000_000;
const INITIAL_Y: u64 = 200_000_000;

#[derive(Clone)]
pub struct Pool {
    pub seed: u64,
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    pub config: Pubkey,
    pub mint_lp: Pubkey,
    pub vault_x: Pubkey,
    pub vault_y: Pubkey,
    pub treasury: Pubkey,
}

impl Pool {
    fn derive(seed: u64, mint_x: Pubkey, mint_y: Pubkey, treasury: Pubkey) -> Self {
        let config =
            Pubkey::find_program_address(&[b"config", &seed.to_le_bytes()], &amm_video::id()).0;
        let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;

        // Derive the PDA for the vault associated token account using the config PDA and Mint A
        Pool {
            seed,
            mint_x,
            mint_y,
            config,
            mint_lp,
            vault_x: associated_token::get_associated_token_address(&config, &mint_x),
            vault_y: associated_token::get_associated_token_address(&config, &mint_y),
            treasury,
        }
    }

    fn user_x(&self, user: &Pubkey) -> Pubkey {
        associated_token::get_associated_token_address(user, &self.mint_x)
    }

    fn user_y(&self, user: &Pubkey) -> Pubkey {
        associated_token::get_associated_token_address(user, &self.mint_y)
    }

    fn user_lp(&self, user: &Pubkey) -> Pubkey {
        associated_token::get_associated_token_address(user, &self.mint_lp)
    }

    fn treasury_x(&self) -> Pubkey {
        associated_token::get_associated_token_address(&self.treasury, &self.mint_x)
    }

    fn treasury_y(&self) -> Pubkey {
        associated_token::get_associated_token_address(&self.treasury, &self.mint_y)
    }
}

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (LiteSVM, Keypair, Pool) {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(amm_video::id(), bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();
    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let treasury = Keypair::new().pubkey();
    let pool = Pool::derive(SEED, mint_x, mint_y, treasury);

    (svm, payer, pool)
}

fn fund_user(svm: &mut LiteSVM, mint_authority: &Keypair, user: &Pubkey, pool: &Pool) {
    for mint in [pool.mint_x, pool.mint_y] {
        let ata = CreateAssociatedTokenAccount::new(svm, mint_authority, &mint)
            .owner(user)
            .send()
            .unwrap();
        MintTo::new(svm, mint_authority, &mint, &ata, USER_FUNDS)
            .send()
            .unwrap();
    }
}

fn init_pool(svm: &mut LiteSVM, payer: &Keypair, pool: &Pool, authority: Option<Pubkey>) {
    let ix = create_initialise_ix(payer.pubkey(), pool, FEE, PROTOCOL_FEE, authority);
    send(svm, &[ix], payer, &[payer]).unwrap();
}

fn setup_funded_pool() -> (LiteSVM, Keypair, Pool) {
    let (mut svm, payer, pool) = setup();
    init_pool(&mut svm, &payer, &pool, Some(payer.pubkey()));
    fund_user(&mut svm, &payer, &payer.pubkey(), &pool);

    let ix = create_deposit_ix(payer.pubkey(), &pool, INITIAL_LP, INITIAL_X, INITIAL_Y);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();

    (svm, payer, pool)
}

fn balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    match svm.get_account(token_account) {
        Some(account) => {
            TokenAccount::try_deserialize(&mut account.data.as_slice())
                .unwrap()
                .amount
        }
        None => 0,
    }
}

fn lp_supply(svm: &LiteSVM, pool: &Pool) -> u64 {
    let account = svm.get_account(&pool.mint_lp).unwrap();
    Mint::try_deserialize(&mut account.data.as_slice())
        .unwrap()
        .supply
}

fn read_config(svm: &LiteSVM, pool: &Pool) -> Config {
    let account = svm.get_account(&pool.config).unwrap();
    Config::try_deserialize(&mut account.data.as_slice()).unwrap()
}

fn assert_amm_error(result: TransactionResult, expected: AmmError) {
    let name = expected.to_string();
    let code: u32 = expected.into();
    let failed = result.expect_err("transaction should have failed");
    let err = format!("{:?}", failed.err);
    assert!(
        err.contains(&format!("Custom({})", code)),
        "expected \"{}\" (Custom({})), got: {}\nlogs: {:#?}",
        name,
        code,
        err,
        failed.meta.logs
    );
}

fn expected_swap(reserve_in: u64, reserve_out: u64, amount: u64) -> (u64, u64, u64) {
    let cut = amount * PROTOCOL_FEE as u64 / 10_000;
    let into_vault = amount - cut;
    let after_lp_fee = into_vault as u128 * (10_000 - FEE as u128) / 10_000;
    let k = reserve_in as u128 * reserve_out as u128;
    let out = reserve_out as u128 - k / (reserve_in as u128 + after_lp_fee);
    (cut, into_vault, out as u64)
}

#[test]
fn test_initialize() {
    let (mut svm, payer, pool) = setup();
    init_pool(&mut svm, &payer, &pool, Some(payer.pubkey()));

    let config = read_config(&svm, &pool);
    assert_eq!(config.seed, SEED);
    assert_eq!(config.authority, Some(payer.pubkey()));
    assert_eq!(config.treasury, pool.treasury);
    assert_eq!(config.mint_x, pool.mint_x);
    assert_eq!(config.mint_y, pool.mint_y);
    assert_eq!(config.fee, FEE);
    assert_eq!(config.protocol_fee, PROTOCOL_FEE);
    assert!(!config.locked);

    assert!(svm.get_account(&pool.vault_x).is_some());
    assert!(svm.get_account(&pool.vault_y).is_some());
    assert_eq!(balance(&svm, &pool.vault_x), 0);
    assert_eq!(balance(&svm, &pool.vault_y), 0);
    assert_eq!(lp_supply(&svm, &pool), 0);
}

#[test]
fn test_initialize_fee_too_high_fails() {
    let (mut svm, payer, pool) = setup();

    let ix = create_initialise_ix(payer.pubkey(), &pool, 9_990, 20, Some(payer.pubkey()));
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::FeePercentErr);
    assert!(svm.get_account(&pool.config).is_none());
}

#[test]
fn test_initialize_same_mint_fails() {
    let (mut svm, payer, pool) = setup();
    let same_mint_pool = Pool::derive(SEED, pool.mint_x, pool.mint_x, pool.treasury);

    let ix = create_initialise_ix(
        payer.pubkey(),
        &same_mint_pool,
        FEE,
        PROTOCOL_FEE,
        Some(payer.pubkey()),
    );
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert!(res.is_err(), "a pool of a token against itself must fail");
    assert!(svm.get_account(&same_mint_pool.config).is_none());
}

#[test]
fn test_first_deposit() {
    let (svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();

    assert_eq!(balance(&svm, &pool.vault_x), INITIAL_X);
    assert_eq!(balance(&svm, &pool.vault_y), INITIAL_Y);
    assert_eq!(balance(&svm, &pool.user_x(&user)), USER_FUNDS - INITIAL_X);
    assert_eq!(balance(&svm, &pool.user_y(&user)), USER_FUNDS - INITIAL_Y);
    assert_eq!(balance(&svm, &pool.user_lp(&user)), INITIAL_LP);
    assert_eq!(lp_supply(&svm, &pool), INITIAL_LP);
}

#[test]
fn test_second_deposit_is_proportional() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();

    let ix = create_deposit_ix(user, &pool, INITIAL_LP / 2, 100_000_000, 100_000_000);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();

    assert_eq!(balance(&svm, &pool.vault_x), 300_000_000);
    assert_eq!(balance(&svm, &pool.vault_y), 300_000_000);
    assert_eq!(balance(&svm, &pool.user_lp(&user)), 150_000_000);
    assert_eq!(lp_supply(&svm, &pool), 150_000_000);
}

#[test]
fn test_deposit_slippage_fails() {
    let (mut svm, payer, pool) = setup_funded_pool();

    let ix = create_deposit_ix(payer.pubkey(), &pool, INITIAL_LP / 2, 99_999_999, 100_000_000);
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::SlippageExceeded);
    assert_eq!(balance(&svm, &pool.vault_x), INITIAL_X);
    assert_eq!(lp_supply(&svm, &pool), INITIAL_LP);
}

#[test]
fn test_deposit_zero_amount_fails() {
    let (mut svm, payer, pool) = setup_funded_pool();

    let ix = create_deposit_ix(payer.pubkey(), &pool, 0, 100_000_000, 100_000_000);
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::InvalidAmount);
}

#[test]
fn test_withdraw() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();

    let ix = create_withdraw_ix(user, &pool, 10_000_000, 20_000_000, 20_000_000);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();

    assert_eq!(balance(&svm, &pool.vault_x), 180_000_000);
    assert_eq!(balance(&svm, &pool.vault_y), 180_000_000);
    assert_eq!(
        balance(&svm, &pool.user_x(&user)),
        USER_FUNDS - INITIAL_X + 20_000_000
    );
    assert_eq!(
        balance(&svm, &pool.user_y(&user)),
        USER_FUNDS - INITIAL_Y + 20_000_000
    );
    assert_eq!(balance(&svm, &pool.user_lp(&user)), 90_000_000);
    assert_eq!(lp_supply(&svm, &pool), 90_000_000);
}

#[test]
fn test_withdraw_slippage_fails() {
    let (mut svm, payer, pool) = setup_funded_pool();

    let ix = create_withdraw_ix(payer.pubkey(), &pool, 10_000_000, 20_000_001, 20_000_000);
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::SlippageExceeded);
    assert_eq!(lp_supply(&svm, &pool), INITIAL_LP);
    assert_eq!(balance(&svm, &pool.vault_x), INITIAL_X);
}

#[test]
fn test_swap_x_for_y() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();
    let amount = 10_000_000;

    let (cut, into_vault, out) = expected_swap(INITIAL_X, INITIAL_Y, amount);
    let user_x_before = balance(&svm, &pool.user_x(&user));
    let user_y_before = balance(&svm, &pool.user_y(&user));

    let ix = create_swap_ix(user, &pool, true, amount, out);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();

    assert_eq!(balance(&svm, &pool.user_x(&user)), user_x_before - amount);
    assert_eq!(balance(&svm, &pool.user_y(&user)), user_y_before + out);

    assert_eq!(balance(&svm, &pool.treasury_x()), cut);
    assert_eq!(balance(&svm, &pool.treasury_y()), 0);
    assert_eq!(balance(&svm, &pool.vault_x), INITIAL_X + into_vault);
    assert_eq!(balance(&svm, &pool.vault_y), INITIAL_Y - out);

    let k_before = INITIAL_X as u128 * INITIAL_Y as u128;
    let k_after = balance(&svm, &pool.vault_x) as u128 * balance(&svm, &pool.vault_y) as u128;
    assert!(k_after > k_before);
}

#[test]
fn test_swap_y_for_x() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();
    let amount = 10_000_000;

    let (cut, into_vault, out) = expected_swap(INITIAL_Y, INITIAL_X, amount);
    let user_x_before = balance(&svm, &pool.user_x(&user));
    let user_y_before = balance(&svm, &pool.user_y(&user));

    let ix = create_swap_ix(user, &pool, false, amount, out);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();

    assert_eq!(balance(&svm, &pool.user_y(&user)), user_y_before - amount);
    assert_eq!(balance(&svm, &pool.user_x(&user)), user_x_before + out);
    assert_eq!(balance(&svm, &pool.treasury_y()), cut);
    assert_eq!(balance(&svm, &pool.treasury_x()), 0);
    assert_eq!(balance(&svm, &pool.vault_y), INITIAL_Y + into_vault);
    assert_eq!(balance(&svm, &pool.vault_x), INITIAL_X - out);
}

#[test]
fn test_swap_slippage_fails() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let amount = 10_000_000;
    let (_, _, out) = expected_swap(INITIAL_X, INITIAL_Y, amount);

    let ix = create_swap_ix(payer.pubkey(), &pool, true, amount, out + 1);
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::SlippageExceeded);
    assert_eq!(balance(&svm, &pool.vault_x), INITIAL_X);
    assert_eq!(balance(&svm, &pool.vault_y), INITIAL_Y);
}

#[test]
fn test_swap_wrong_treasury_fails() {
    let (mut svm, payer, pool) = setup_funded_pool();

    let mut fake = pool.clone();
    fake.treasury = Keypair::new().pubkey();

    let ix = create_swap_ix(payer.pubkey(), &fake, true, 10_000_000, 1);
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::InvalidTreasury);
}

#[test]
fn test_lock_blocks_deposit_withdraw_and_swap() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();

    let ix = create_lock_ix(user, &pool);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();
    assert!(read_config(&svm, &pool).locked);

    let ix = create_deposit_ix(user, &pool, 1_000_000, 10_000_000, 10_000_000);
    assert_amm_error(send(&mut svm, &[ix], &payer, &[&payer]), AmmError::PoolLocked);

    let ix = create_withdraw_ix(user, &pool, 1_000_000, 0, 0);
    assert_amm_error(send(&mut svm, &[ix], &payer, &[&payer]), AmmError::PoolLocked);

    let ix = create_swap_ix(user, &pool, true, 1_000_000, 0);
    assert_amm_error(send(&mut svm, &[ix], &payer, &[&payer]), AmmError::PoolLocked);
}

#[test]
fn test_unlock_restores_trading() {
    let (mut svm, payer, pool) = setup_funded_pool();
    let user = payer.pubkey();

    let ix = create_lock_ix(user, &pool);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();
    let ix = create_unlock_ix(user, &pool);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();
    assert!(!read_config(&svm, &pool).locked);

    let ix = create_swap_ix(user, &pool, true, 1_000_000, 0);
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();
}

#[test]
fn test_lock_by_non_authority_fails() {
    let (mut svm, _payer, pool) = setup_funded_pool();

    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    let ix = create_lock_ix(stranger.pubkey(), &pool);
    let res = send(&mut svm, &[ix], &stranger, &[&stranger]);

    assert_amm_error(res, AmmError::InvalidAuthority);
    assert!(!read_config(&svm, &pool).locked);
}

#[test]
fn test_lock_without_authority_fails() {
    let (mut svm, payer, pool) = setup();
    init_pool(&mut svm, &payer, &pool, None);

    let ix = create_lock_ix(payer.pubkey(), &pool);
    let res = send(&mut svm, &[ix], &payer, &[&payer]);

    assert_amm_error(res, AmmError::NoAuthoritySet);
    assert!(!read_config(&svm, &pool).locked);
}
