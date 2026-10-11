# remit-stablecoin

Week 4 assignment for Turbin3 (Q3 2026 cohort). A Token-2022 stablecoin for remittances, built as an Anchor program that sets up the mint and drives its extensions through CPIs:

- a transfer fee on every transfer, with a cap
- KYC gating: every new token account starts frozen and the issuer thaws it once the holder is verified
- a metadata pointer and a close authority on the mint
- a second, confidential version of the mint with a seizure authority (permanent delegate) and confidential transfers

Built on Anchor 1.1.2 and tested against [LiteSVM](https://github.com/LiteSVM/litesvm), which ships with a real Token-2022 build, so the suite is a plain `cargo test`.

## The instructions

| Instruction | What it does |
|---|---|
| `initialize_mint` | Creates a mint with TransferFeeConfig, MetadataPointer (pointing at the mint itself), DefaultAccountState set to Frozen, and MintCloseAuthority |
| `transfer_with_fee` | Reads the mint through `StateWithExtensions`, works out the fee for the current epoch, and calls `transfer_checked_with_fee` |
| `thaw_account` | Thaws one frozen token account. Only the mint's freeze authority can call it |
| `initialize_mint_confidential` | Creates a mint with the same four extensions plus PermanentDelegate, ConfidentialTransferMint and ConfidentialTransferFeeConfig |

The issuer that signs `initialize_mint` holds every authority on the mint: mint, freeze, transfer fee config, withdraw withheld, metadata pointer, close and, on the confidential mint, permanent delegate and confidential transfer authority.

## How the tasks map

| Task | Where |
|---|---|
| 1. Mint with four extensions, sized with `try_calculate_account_len`, every extension initialized before `InitializeMint2` | `instructions/extensions.rs`, `initialize_mint.rs` |
| 2. Transfer that recomputes the fee from `calculate_epoch_fee` on every call | `transfer_with_fee.rs` |
| 3. Mint and account state read only through `StateWithExtensions` | `transfer_with_fee.rs`, `thaw_account.rs`, and every check in the tests |
| 4. KYC thaw of a single account, gated on the freeze authority | `thaw_account.rs` |
| 5. Confidential mint with a seizure authority | `initialize_mint_confidential.rs` |
| 6. Full confidential transfer lifecycle | Not done, see below |

## Building a mint with extensions

A Token-2022 mint with extensions is created in three steps, and the order is fixed:

1. `create_account` with the full size, from `ExtensionType::try_calculate_account_len::<Mint>(&extensions)`. Any other number and the mint is too small to initialize.
2. One initialize instruction per extension, while the account is still an uninitialized mint.
3. `InitializeMint2`, which seals it. Most mint extensions cannot be added after this.

`MintAccounts` in `extensions.rs` holds those three steps so both mints share them. The confidential mint calls the same `allocate`, `init_base_extensions` and `finish`, with three more extensions in between.

## Build and test

The Anchor CLI isn't needed. The tests load the `.so` directly.

```bash
cargo build-sbf --manifest-path programs/remit-stablecoin/Cargo.toml --sbf-out-dir target/deploy
```

```bash
cargo test
```

Build first, always. `tests/stablecoin.rs` does `include_bytes!` on `target/deploy/remit_stablecoin.so` at compile time, so a missing or stale build shows up as a compile error rather than a test failure.

### Proof

![cargo test](screenshots/cargo_test.png)

## The tests

| Test | Covers |
|---|---|
| `initialize_mint_stacks_four_extensions` | Exactly the four extensions, read back with `StateWithExtensions`: fee rate and cap, metadata pointer at the mint, default state Frozen, close authority |
| `initialize_mint_rejects_a_fee_above_100_percent` | `FeeTooHigh` for 10,001 basis points |
| `new_token_accounts_start_frozen` | A fresh token account is Frozen and cannot receive a mint |
| `freeze_authority_thaws_one_account_at_a_time` | Thawing Alice leaves Bob frozen, and Alice can now receive tokens |
| `thaw_rejects_anyone_but_the_freeze_authority` | `NotFreezeAuthority`, account still frozen |
| `thaw_rejects_an_account_that_is_not_frozen` | `AccountNotFrozen` on a second thaw |
| `transfer_with_fee_withholds_the_epoch_fee` | 100 tokens at 0.5%: sender down 100, receiver gets 99.5, 0.5 withheld on the receiver's account |
| `transfer_with_fee_caps_at_the_maximum_fee` | 1,000 tokens would be a 5 token fee, capped at 2 |
| `transfer_with_fee_only_moves_the_owners_tokens` | Someone else cannot move Alice's tokens through the program |
| `confidential_mint_keeps_the_base_extensions_and_adds_three` | All seven extensions, permanent delegate is the issuer, manual account approval, withheld-fee ElGamal key stored |
| `permanent_delegate_can_seize_without_the_holders_signature` | The issuer moves a holder's whole balance to a treasury account with only its own signature |

## Task 6: the confidential lifecycle

Not implemented. Configuring an account, transferring and withdrawing confidentially all need zero-knowledge proofs (pubkey validity, equality, ciphertext validity, range) that are generated client-side and verified by the ZK ElGamal proof program. LiteSVM does not include that program, so none of it could be tested here; it would need a devnet run with a client that builds the proofs. Deposit and apply-pending-balance need no proofs, but both require a configured account first, so they hit the same wall.

## Notes and gotchas

- **The typed mint hides the extensions.** `InterfaceAccount<Mint>` runs `StateWithExtensions::unpack` and keeps only the base mint, so decimals and authorities are there but the transfer fee is not. `transfer_with_fee` and `thaw_account` re-borrow the raw bytes and unpack them again to get at the extensions.
- **The fee is not cached.** The fee authority can schedule a new rate that takes effect two epochs later, so `transfer_with_fee` asks `calculate_epoch_fee` with the live `Clock` epoch every time. `transfer_checked_with_fee` fails if the fee passed in does not match what Token-2022 works out itself.
- **The fee is withheld from the receiver.** The sender pays `amount`, the receiver gets `amount - fee`, and the fee sits on the receiver's account in `TransferFeeAmount` until it is harvested.
- **A default state of Frozen needs a freeze authority.** `InitializeMint2` rejects the mint otherwise, so `finish` always passes the issuer as freeze authority.
- **Confidential transfers plus a transfer fee need a third extension.** With both TransferFeeConfig and ConfidentialTransferMint on the mint, Token-2022 also requires ConfidentialTransferFeeConfig. A fee in the clear would reveal the hidden amount, so the withheld fee is encrypted under the fee authority's own ElGamal key.
- **No Anchor helper for the confidential extensions.** `anchor-spl` 1.1.2 ships empty `confidential_transfer` modules, so `initialize_mint_confidential` builds those two instructions with `spl-token-2022-interface` and calls `invoke` directly.
- **Confidential extensions only exist at creation.** They cannot be added to the first mint later, which is why task 5 creates a second mint and carries the four base extensions over.
- **Thaw checks the freeze authority itself.** Token-2022 would reject the wrong signer anyway, but checking first gives a readable `NotFreezeAuthority` instead of a generic token error.

## Reference docs

- [Token-2022 extensions guide](https://solana.com/developers/guides/token-extensions/getting-started)
- [Transfer fees](https://solana.com/developers/guides/token-extensions/transfer-fee), [Default account state](https://solana.com/developers/guides/token-extensions/default-account-state), [Permanent delegate](https://solana.com/developers/guides/token-extensions/permanent-delegate)
- [Confidential transfers](https://solana.com/docs/tokens/extensions/confidential-transfer)
- [LiteSVM](https://litesvm.dev/)
