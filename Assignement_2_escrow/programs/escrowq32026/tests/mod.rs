use {
    anchor_lang::{
        prelude::{msg, Clock},
        solana_program::instruction::Instruction,
        solana_program::program_pack::Pack,
        system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
        token::spl_token,
    },
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::Keypair,
    solana_message::Message,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (LiteSVM, Keypair) {
    let program_id = escrowq32026::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Return the LiteSVM instance and payer keypair
    (svm, payer)
}

#[test]
fn test_make_and_refund() {
    // Setup the test environment by initializing LiteSVM and creating a payer keypair
    let (mut program, payer) = setup();

    // Get the maker's public key from the payer keypair
    let maker = payer.pubkey();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_a = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint A: {}\n", mint_a);

    let mint_b = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint B: {}\n", mint_b);

    // Create the maker's associated token account for Mint A
    // This is done using litesvm-token's CreateAssociatedTokenAccount utility
    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();
    msg!("Maker ATA A: {}\n", maker_ata_a);

    // Derive the PDA for the escrow account using the maker's public key and a seed value
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;
    msg!("Escrow PDA: {}\n", escrow);

    // Derive the PDA for the vault associated token account using the escrow PDA and Mint A
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    msg!("Vault PDA: {}\n", vault);

    // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
    MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    // Create the "Make" instruction to deposit tokens into the escrow
    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker: maker,
            mint_a: mint_a,
            mint_b: mint_b,
            maker_ata_a: maker_ata_a,
            escrow: escrow,
            vault: vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: 10_000_000,
            seed: 123u64,
            receive: 10_000_000,
            expiration: 17780206209,
        }
        .data(),
    };

    // Create and send the transaction containing the "Make" instruction
    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();

    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    // Send the transaction and capture the result
    let tx = program.send_transaction(transaction).unwrap();

    // Log transaction details
    msg!("\n\nMake transaction sucessfull");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Verify the vault account and escrow account data after the "Make" instruction
    let vault_account = program.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10_000_000);
    assert_eq!(vault_data.owner, escrow);
    assert_eq!(vault_data.mint, mint_a);

    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.seed, 123u64);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.receive, 10_000_000);

    // Create the "Refund" instruction to refund tokens back to the maker
    let refund_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Refund {
            maker: maker,
            mint_a: mint_a,
            maker_ata_a: maker_ata_a,
            escrow: escrow,
            vault: vault,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Refund {}.data(),
    };

    // Create and send the transaction containing the "Refund" instruction
    let message = Message::new(&[refund_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();

    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    // Send the transaction and capture the result
    let tx = program.send_transaction(transaction).unwrap();

    // Log transaction details
    msg!("\n\nRefund transaction sucessful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());
}

// Shared setup for the take tests: two mints, a funded maker and taker, and an escrow
// already created by `make`. Returns everything the take instruction needs.
struct TakeFixture {
    taker: Keypair,
    maker: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    taker_ata_a: Pubkey,
    taker_ata_b: Pubkey,
    maker_ata_b: Pubkey,
    escrow: Pubkey,
    vault: Pubkey,
}

const DEPOSIT: u64 = 10_000_000;
const RECEIVE: u64 = 10_000_000;

fn setup_escrow(program: &mut LiteSVM, payer: &Keypair, expiration: i64) -> TakeFixture {
    let maker = payer.pubkey();

    // The taker is a second party with its own SOL, since it pays for the ATAs the take
    // instruction creates, so it needs a balance of its own.
    let taker = Keypair::new();
    program.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();

    let mint_a = CreateMint::new(program, payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    let mint_b = CreateMint::new(program, payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    // The maker holds token A and the taker holds token B, the two sides of the swap.
    let maker_ata_a = CreateAssociatedTokenAccount::new(program, payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();
    MintTo::new(program, payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    let taker_ata_b = CreateAssociatedTokenAccount::new(program, payer, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(program, payer, &mint_b, &taker_ata_b, 1000_000_000)
        .send()
        .unwrap();

    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    // These two are created by the take instruction itself (`init_if_needed`),
    // so we only derive their addresses here.
    let taker_ata_a = associated_token::get_associated_token_address(&taker.pubkey(), &mint_a);
    let maker_ata_b = associated_token::get_associated_token_address(&maker, &mint_b);

    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            seed: 123u64,
            deposit: DEPOSIT,
            receive: RECEIVE,
            expiration,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[payer], message, recent_blockhash);
    program.send_transaction(transaction).unwrap();

    TakeFixture {
        taker,
        maker,
        mint_a,
        mint_b,
        taker_ata_a,
        taker_ata_b,
        maker_ata_b,
        escrow,
        vault,
    }
}

// Build the take instruction for a fixture. The taker signs, the payer funds the tx.
fn take_ix(f: &TakeFixture) -> Instruction {
    Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Take {
            taker: f.taker.pubkey(),
            maker: f.maker,
            mint_a: f.mint_a,
            mint_b: f.mint_b,
            taker_ata_a: f.taker_ata_a,
            taker_ata_b: f.taker_ata_b,
            maker_ata_b: f.maker_ata_b,
            escrow: f.escrow,
            vault: f.vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Take {}.data(),
    }
}

#[test]
fn test_make_and_take() {
    let (mut program, payer) = setup();

    // Year 2533, comfortably in the future, so the expiration check passes.
    let f = setup_escrow(&mut program, &payer, 17780206209);

    let message = Message::new(&[take_ix(&f)], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();

    // Both the fee payer and the taker must sign: the taker is the transfer authority
    // for token B and the payer for the two ATAs the instruction creates.
    let transaction = Transaction::new(&[&payer, &f.taker], message, recent_blockhash);
    let tx = program.send_transaction(transaction).unwrap();

    msg!("\n\nTake transaction sucessful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // The taker received token A out of the vault...
    let taker_ata_a_account = program.get_account(&f.taker_ata_a).unwrap();
    let taker_ata_a_data = spl_token::state::Account::unpack(&taker_ata_a_account.data).unwrap();
    assert_eq!(taker_ata_a_data.amount, DEPOSIT);
    assert_eq!(taker_ata_a_data.mint, f.mint_a);
    assert_eq!(taker_ata_a_data.owner, f.taker.pubkey());

    // ...and the maker received token B from the taker.
    let maker_ata_b_account = program.get_account(&f.maker_ata_b).unwrap();
    let maker_ata_b_data = spl_token::state::Account::unpack(&maker_ata_b_account.data).unwrap();
    assert_eq!(maker_ata_b_data.amount, RECEIVE);
    assert_eq!(maker_ata_b_data.mint, f.mint_b);
    assert_eq!(maker_ata_b_data.owner, f.maker);

    // Both program-owned accounts are closed and their rent returned.
    assert!(program.get_account(&f.escrow).is_none());
    assert!(program.get_account(&f.vault).is_none());
}

#[test]
fn test_take_after_expiry() {
    let (mut program, payer) = setup();

    let expiration = 1_000_000_000i64;
    let f = setup_escrow(&mut program, &payer, expiration);

    // LiteSVM's default clock starts at unix_timestamp 0, so an expiration in the past
    // has to be produced by moving the clock forward rather than by picking a small value.
    let mut clock = program.get_sysvar::<Clock>();
    assert!(
        clock.unix_timestamp <= expiration,
        "clock should start before the expiration"
    );
    clock.unix_timestamp = expiration + 1;
    program.set_sysvar::<Clock>(&clock);

    let message = Message::new(&[take_ix(&f)], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer, &f.taker], message, recent_blockhash);

    let failed = program.send_transaction(transaction).unwrap_err();

    // Confirm the failure is the expiration check and not something incidental.
    let expected: u32 = escrowq32026::error::ErrorCode::EscrowExpired.into();
    let err = format!("{:?}", failed.err);
    assert!(
        err.contains(&format!("Custom({})", expected)),
        "expected EscrowExpired (Custom({})), got: {}",
        expected,
        err
    );
    assert!(
        failed.meta.logs.iter().any(|l| l.contains("EscrowExpired")),
        "expected EscrowExpired in logs, got: {:#?}",
        failed.meta.logs
    );

    // Nothing moved: the escrow and vault are untouched.
    assert!(program.get_account(&f.escrow).is_some());
    let vault_account = program.get_account(&f.vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, DEPOSIT);
}
