use {
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id,
            spl_associated_token_account::instruction::create_associated_token_account,
        },
        token_2022::spl_token_2022,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint,
            confidential_transfer_fee::ConfidentialTransferFeeConfig,
            default_account_state::DefaultAccountState,
            metadata_pointer::MetadataPointer,
            mint_close_authority::MintCloseAuthority,
            permanent_delegate::PermanentDelegate,
            transfer_fee::{TransferFeeAmount, TransferFeeConfig},
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
        instruction as token_instruction,
        solana_zk_sdk::encryption::{elgamal::ElGamalKeypair, pod::elgamal::PodElGamalPubkey},
        state::{Account, AccountState, Mint},
        ID as TOKEN_2022_ID,
    },
};

const DECIMALS: u8 = 6;
const FEE_BASIS_POINTS: u16 = 50;
const MAXIMUM_FEE: u64 = 2_000_000;
const ONE_TOKEN: u64 = 1_000_000;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/remit_stablecoin.so"
    ));
    svm.add_program(remit_stablecoin::id(), bytes).unwrap();
    let issuer = Keypair::new();
    svm.airdrop(&issuer.pubkey(), 10_000_000_000).unwrap();
    (svm, issuer)
}

fn send(svm: &mut LiteSVM, ixs: &[Instruction], payer: &Keypair, extra: &[&Keypair]) -> TransactionResult {
    let mut signers = vec![payer];
    signers.extend_from_slice(extra);
    let message = Message::new(ixs, Some(&payer.pubkey()));
    let tx = Transaction::new(&signers, message, svm.latest_blockhash());
    let result = svm.send_transaction(tx);
    svm.expire_blockhash();
    result
}

fn assert_fails_with(result: TransactionResult, needle: &str) {
    let failed = result.expect_err("transaction should have failed");
    let logs = failed.meta.logs.join("\n");
    assert!(logs.contains(needle), "expected `{needle}` in logs:\n{logs}");
}

fn initialize_mint_ix(issuer: &Pubkey, mint: &Pubkey, fee_basis_points: u16) -> Instruction {
    Instruction {
        program_id: remit_stablecoin::id(),
        accounts: remit_stablecoin::accounts::InitializeMint {
            issuer: *issuer,
            mint: *mint,
            token_program: TOKEN_2022_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: remit_stablecoin::instruction::InitializeMint {
            decimals: DECIMALS,
            fee_basis_points,
            maximum_fee: MAXIMUM_FEE,
        }
        .data(),
    }
}

fn create_mint(svm: &mut LiteSVM, issuer: &Keypair) -> Pubkey {
    let mint = Keypair::new();
    let ix = initialize_mint_ix(&issuer.pubkey(), &mint.pubkey(), FEE_BASIS_POINTS);
    send(svm, &[ix], issuer, &[&mint]).unwrap();
    mint.pubkey()
}

fn create_confidential_mint(svm: &mut LiteSVM, issuer: &Keypair, elgamal: [u8; 32]) -> Pubkey {
    let mint = Keypair::new();
    let ix = Instruction {
        program_id: remit_stablecoin::id(),
        accounts: remit_stablecoin::accounts::InitializeMintConfidential {
            issuer: issuer.pubkey(),
            mint: mint.pubkey(),
            token_program: TOKEN_2022_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: remit_stablecoin::instruction::InitializeMintConfidential {
            decimals: DECIMALS,
            fee_basis_points: FEE_BASIS_POINTS,
            maximum_fee: MAXIMUM_FEE,
            withdraw_withheld_elgamal_pubkey: elgamal,
        }
        .data(),
    };
    send(svm, &[ix], issuer, &[&mint]).unwrap();
    mint.pubkey()
}

fn create_token_account(svm: &mut LiteSVM, payer: &Keypair, owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ix = create_associated_token_account(&payer.pubkey(), owner, mint, &TOKEN_2022_ID);
    send(svm, &[ix], payer, &[]).unwrap();
    get_associated_token_address_with_program_id(owner, mint, &TOKEN_2022_ID)
}

fn thaw(svm: &mut LiteSVM, authority: &Keypair, mint: &Pubkey, account: &Pubkey) -> TransactionResult {
    let ix = Instruction {
        program_id: remit_stablecoin::id(),
        accounts: remit_stablecoin::accounts::ThawAccount {
            freeze_authority: authority.pubkey(),
            mint: *mint,
            token_account: *account,
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
        data: remit_stablecoin::instruction::ThawAccount {}.data(),
    };
    send(svm, &[ix], authority, &[])
}

fn mint_to(svm: &mut LiteSVM, issuer: &Keypair, mint: &Pubkey, account: &Pubkey, amount: u64) -> TransactionResult {
    let ix = token_instruction::mint_to_checked(
        &TOKEN_2022_ID,
        mint,
        account,
        &issuer.pubkey(),
        &[],
        amount,
        DECIMALS,
    )
    .unwrap();
    send(svm, &[ix], issuer, &[])
}

fn transfer_with_fee(
    svm: &mut LiteSVM,
    owner: &Keypair,
    mint: &Pubkey,
    source: &Pubkey,
    destination: &Pubkey,
    amount: u64,
) -> TransactionResult {
    let ix = Instruction {
        program_id: remit_stablecoin::id(),
        accounts: remit_stablecoin::accounts::TransferWithFee {
            owner: owner.pubkey(),
            mint: *mint,
            source: *source,
            destination: *destination,
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
        data: remit_stablecoin::instruction::TransferWithFee { amount }.data(),
    };
    send(svm, &[ix], owner, &[])
}

fn data(svm: &LiteSVM, address: &Pubkey) -> Vec<u8> {
    svm.get_account(address).unwrap().data
}

// (balance, state, fee withheld on the account)
fn token_account(svm: &LiteSVM, address: &Pubkey) -> (u64, AccountState, u64) {
    let bytes = data(svm, address);
    let account = StateWithExtensions::<Account>::unpack(&bytes).unwrap();
    let withheld = account.get_extension::<TransferFeeAmount>().unwrap().withheld_amount;
    (account.base.amount, account.base.state, u64::from(withheld))
}

fn funded_user(svm: &mut LiteSVM, issuer: &Keypair, mint: &Pubkey, amount: u64) -> (Keypair, Pubkey) {
    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 1_000_000_000).unwrap();
    let account = create_token_account(svm, issuer, &user.pubkey(), mint);
    thaw(svm, issuer, mint, &account).unwrap();
    if amount > 0 {
        mint_to(svm, issuer, mint, &account, amount).unwrap();
    }
    (user, account)
}

#[test]
fn initialize_mint_stacks_four_extensions() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);

    let bytes = data(&svm, &mint);
    let state = StateWithExtensions::<Mint>::unpack(&bytes).unwrap();
    let mut types = state.get_extension_types().unwrap();
    types.sort_by_key(|t| *t as u16);
    let mut expected = vec![
        ExtensionType::TransferFeeConfig,
        ExtensionType::MetadataPointer,
        ExtensionType::DefaultAccountState,
        ExtensionType::MintCloseAuthority,
    ];
    expected.sort_by_key(|t| *t as u16);
    assert_eq!(types, expected);

    assert_eq!(state.base.decimals, DECIMALS);
    assert_eq!(state.base.mint_authority, Some(issuer.pubkey()).into());
    assert_eq!(state.base.freeze_authority, Some(issuer.pubkey()).into());

    let fee = state.get_extension::<TransferFeeConfig>().unwrap();
    assert_eq!(u16::from(fee.newer_transfer_fee.transfer_fee_basis_points), FEE_BASIS_POINTS);
    assert_eq!(u64::from(fee.newer_transfer_fee.maximum_fee), MAXIMUM_FEE);

    let pointer = state.get_extension::<MetadataPointer>().unwrap();
    assert_eq!(Option::<Pubkey>::from(pointer.metadata_address), Some(mint));

    let default_state = state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_state.state, AccountState::Frozen as u8);

    let close = state.get_extension::<MintCloseAuthority>().unwrap();
    assert_eq!(Option::<Pubkey>::from(close.close_authority), Some(issuer.pubkey()));
}

#[test]
fn initialize_mint_rejects_a_fee_above_100_percent() {
    let (mut svm, issuer) = setup();
    let mint = Keypair::new();
    let ix = initialize_mint_ix(&issuer.pubkey(), &mint.pubkey(), 10_001);
    assert_fails_with(send(&mut svm, &[ix], &issuer, &[&mint]), "FeeTooHigh");
}

#[test]
fn new_token_accounts_start_frozen() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let user = Keypair::new();
    let account = create_token_account(&mut svm, &issuer, &user.pubkey(), &mint);

    assert_eq!(token_account(&svm, &account).1, AccountState::Frozen);
    assert!(mint_to(&mut svm, &issuer, &mint, &account, ONE_TOKEN).is_err());
    assert_eq!(token_account(&svm, &account).0, 0);
}

#[test]
fn freeze_authority_thaws_one_account_at_a_time() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let alice = create_token_account(&mut svm, &issuer, &Keypair::new().pubkey(), &mint);
    let bob = create_token_account(&mut svm, &issuer, &Keypair::new().pubkey(), &mint);

    thaw(&mut svm, &issuer, &mint, &alice).unwrap();

    assert_eq!(token_account(&svm, &alice).1, AccountState::Initialized);
    assert_eq!(token_account(&svm, &bob).1, AccountState::Frozen);
    mint_to(&mut svm, &issuer, &mint, &alice, ONE_TOKEN).unwrap();
    assert_eq!(token_account(&svm, &alice).0, ONE_TOKEN);
}

#[test]
fn thaw_rejects_anyone_but_the_freeze_authority() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let account = create_token_account(&mut svm, &issuer, &Keypair::new().pubkey(), &mint);
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    assert_fails_with(thaw(&mut svm, &stranger, &mint, &account), "NotFreezeAuthority");
    assert_eq!(token_account(&svm, &account).1, AccountState::Frozen);
}

#[test]
fn thaw_rejects_an_account_that_is_not_frozen() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let account = create_token_account(&mut svm, &issuer, &Keypair::new().pubkey(), &mint);

    thaw(&mut svm, &issuer, &mint, &account).unwrap();
    assert_fails_with(thaw(&mut svm, &issuer, &mint, &account), "AccountNotFrozen");
}

#[test]
fn transfer_with_fee_withholds_the_epoch_fee() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let (alice, alice_account) = funded_user(&mut svm, &issuer, &mint, 1_000 * ONE_TOKEN);
    let (_, bob_account) = funded_user(&mut svm, &issuer, &mint, 0);

    let amount = 100 * ONE_TOKEN;
    let fee = amount * FEE_BASIS_POINTS as u64 / 10_000;
    transfer_with_fee(&mut svm, &alice, &mint, &alice_account, &bob_account, amount).unwrap();

    assert_eq!(token_account(&svm, &alice_account).0, 900 * ONE_TOKEN);
    let (bob_balance, _, bob_withheld) = token_account(&svm, &bob_account);
    assert_eq!(bob_balance, amount - fee);
    assert_eq!(bob_withheld, fee);
}

#[test]
fn transfer_with_fee_caps_at_the_maximum_fee() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let (alice, alice_account) = funded_user(&mut svm, &issuer, &mint, 5_000 * ONE_TOKEN);
    let (_, bob_account) = funded_user(&mut svm, &issuer, &mint, 0);

    // 0.5% of 1,000 tokens is 5 tokens, above the 2 token cap.
    let amount = 1_000 * ONE_TOKEN;
    transfer_with_fee(&mut svm, &alice, &mint, &alice_account, &bob_account, amount).unwrap();

    let (bob_balance, _, bob_withheld) = token_account(&svm, &bob_account);
    assert_eq!(bob_withheld, MAXIMUM_FEE);
    assert_eq!(bob_balance, amount - MAXIMUM_FEE);
}

#[test]
fn transfer_with_fee_only_moves_the_owners_tokens() {
    let (mut svm, issuer) = setup();
    let mint = create_mint(&mut svm, &issuer);
    let (_, alice_account) = funded_user(&mut svm, &issuer, &mint, 100 * ONE_TOKEN);
    let (bob, bob_account) = funded_user(&mut svm, &issuer, &mint, 0);

    let result = transfer_with_fee(&mut svm, &bob, &mint, &alice_account, &bob_account, ONE_TOKEN);
    assert!(result.is_err());
    assert_eq!(token_account(&svm, &alice_account).0, 100 * ONE_TOKEN);
}

#[test]
fn confidential_mint_keeps_the_base_extensions_and_adds_three() {
    let (mut svm, issuer) = setup();
    let elgamal: [u8; 32] = ElGamalKeypair::new_rand().pubkey_owned().into();
    let mint = create_confidential_mint(&mut svm, &issuer, elgamal);

    let bytes = data(&svm, &mint);
    let state = StateWithExtensions::<Mint>::unpack(&bytes).unwrap();
    let types = state.get_extension_types().unwrap();
    for expected in [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MetadataPointer,
        ExtensionType::DefaultAccountState,
        ExtensionType::MintCloseAuthority,
        ExtensionType::PermanentDelegate,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::ConfidentialTransferFeeConfig,
    ] {
        assert!(types.contains(&expected), "missing {expected:?}");
    }

    let delegate = state.get_extension::<PermanentDelegate>().unwrap();
    assert_eq!(Option::<Pubkey>::from(delegate.delegate), Some(issuer.pubkey()));

    let confidential = state.get_extension::<ConfidentialTransferMint>().unwrap();
    assert_eq!(Option::<Pubkey>::from(confidential.authority), Some(issuer.pubkey()));
    assert!(!bool::from(confidential.auto_approve_new_accounts));

    let fee_config = state.get_extension::<ConfidentialTransferFeeConfig>().unwrap();
    assert_eq!(fee_config.withdraw_withheld_authority_elgamal_pubkey, PodElGamalPubkey::from(elgamal));
}

#[test]
fn permanent_delegate_can_seize_without_the_holders_signature() {
    let (mut svm, issuer) = setup();
    let elgamal: [u8; 32] = ElGamalKeypair::new_rand().pubkey_owned().into();
    let mint = create_confidential_mint(&mut svm, &issuer, elgamal);
    let (_, holder_account) = funded_user(&mut svm, &issuer, &mint, 50 * ONE_TOKEN);
    let (_, treasury_account) = funded_user(&mut svm, &issuer, &mint, 0);

    let seize = token_instruction::transfer_checked(
        &TOKEN_2022_ID,
        &holder_account,
        &mint,
        &treasury_account,
        &issuer.pubkey(),
        &[],
        50 * ONE_TOKEN,
        DECIMALS,
    )
    .unwrap();
    send(&mut svm, &[seize], &issuer, &[]).unwrap();

    assert_eq!(token_account(&svm, &holder_account).0, 0);
    let (treasury_balance, _, treasury_withheld) = token_account(&svm, &treasury_account);
    assert_eq!(treasury_balance + treasury_withheld, 50 * ONE_TOKEN);
}
