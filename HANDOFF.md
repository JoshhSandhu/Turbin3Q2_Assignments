# Handoff for the Linux session

Context for Claude Code on Josh's Linux machine, written from the Windows session on 2026-10-12.
Read this first. **`git rm HANDOFF.md` before anything is submitted**; the repo is public.

## Setup

- Josh, Turbin3 Q3 2026 Builders cohort. Repo: `JoshhSandhu/Turbin3Q2_Assignments`.
- Windows is for editing and writing docs. **Linux is where everything builds and runs**: `cargo build-sbf`,
  `anchor`, `surfpool`, `solana`, plus screenshots of passing tests.
- The two machines sync through git. Pull before editing, commit after. Josh pushes.

## How Josh wants things done

- Commit messages: plain, short, **no `Co-Authored-By` or any Claude/Anthropic line**.
- Code and docs: few comments, **no em dashes anywhere**, plain wording, nothing that reads AI-generated.
- By default Josh wants to be guided rather than have assignment code written for him. Week 5 and the
  A3 AMM were written by Claude at his request, so fixing build or test errors in those is fine.
- Claude owes Josh a walkthrough of the Week 5 and A3 code once they're submitted.

## Status

Classroom lists three missing items: the capstone LOI, **Week 5 (Metaplex Core staking)** and the
capstone architecture diagram. Both capstone items are documents handled on Windows with teammate Rohit.
The Linux work is:

| Folder | State | To do on Linux |
|---|---|---|
| `Assignment_1_spl-nft`, `Assignement_2_escrow` | Done | Nothing |
| `Assignment_5_NFT_Staking_Core` | Written on Windows, passes `cargo check` (incl. `--features idl-build`), **never run** | Priority 1: build, test, fix, screenshot |
| `Assignment_3_AMM` | Code and 17 tests committed Sep 16, **never compiled** | Priority 2: build, test, fix, screenshot, README |
| `Assignement_4_Token22CTs` | Placeholder README only (keep the folder spelling) | Backlog. Not on the missing list. Ask Josh before starting |

## Priority 1: Week 5, `Assignment_5_NFT_Staking_Core`

Built on the instructor's `AndreiaCanadas/nft-staking-core`. Stack: **Anchor 0.32.1** (not 1.x like the
other folders), `mpl-core` 0.11.1, TS tests with `@metaplex-foundation/mpl-core` 1.7 and Umi, run on
**Surfpool** (the default `anchor test` validator; it forks mainnet so Metaplex Core is there, needs
internet). Tests move the clock with the `surfnet_timeTravel` RPC (`absoluteTimestamp` in ms, forward only).

```bash
cd Assignment_5_NFT_Staking_Core
avm use 0.32.1                      # avm install 0.32.1 first if needed
surfpool --version || curl -sL https://run.surfpool.run/ | bash
yarn install                        # adds @metaplex-foundation/umi-bundle-defaults to yarn.lock
anchor build
anchor keys sync                    # writes the local program id into lib.rs and Anchor.toml
anchor test
```

What was added on top of the class program:

- `utils.rs`: attribute helpers, reward math, `mint_rewards`, `update_total_staked`.
- NFT attributes: `staked`, `staked_at` (only for the freeze period), `last_claimed_at` (where rewards
  are counted from).
- `claim_rewards`: pays whole days since `last_claimed_at`, advances it by `days * 86400`, NFT stays
  staked and frozen. Errors with `NothingToClaim` if less than a day.
- `unstake`: freeze period from `staked_at`, pays only days since `last_claimed_at`, resets attributes,
  thaws, decrements the collection counter.
- `stake`: also adds a BurnDelegate plugin (owner-managed, so added here with the user as authority and
  `init_authority = UpdateAuthority`, i.e. the PDA) before the FreezeDelegate, then increments the counter.
- `burn_staked_nft`: thaws first (Core will not burn a frozen asset), then `BurnV1` signed by the
  update-authority PDA as BurnDelegate, mints unclaimed days plus `config.burn_bonus`, decrements.
- Collection: Attributes plugin `total_staked`, created in `create_collection`, rewritten with
  `UpdateCollectionPluginV1` signed by the PDA, added if missing.
- `Config` gained `burn_bonus: u64`; `initialize_config` takes it as a third argument.
- Errors added: `NothingToClaim`, `InvalidCounter`.
- Tests: 14, order-dependent. Every staking call adds a 400k compute-unit preInstruction. Plugins are
  read with `fetchAssetV1` / `fetchCollectionV1` (`fetchAsset` would merge the collection's plugins in).

Likely trouble spots: compute limit, Surfpool time travel, the burned-account check (the test accepts
`null` or at most 1 byte of data), the PDA being accepted as burn authority. Fix the cause rather than
loosening the reward rules or the assertions.

When green: save a screenshot of the `anchor test` output to
`Assignment_5_NFT_Staking_Core/screenshots/anchor_test.png` (the README already links it), and commit it
together with the `anchor keys sync` changes and `yarn.lock`. Task 2 (the optional Oracle plugin) is not
done; the README says so.

## Priority 2: A3 AMM, `Assignment_3_AMM`

Anchor 1.0.1 and LiteSVM. `CpiContext::new` taking the program id `Pubkey` is the 1.x API, not a bug.

```bash
cd Assignment_3_AMM
cargo build-sbf --manifest-path programs/amm-video/Cargo.toml --sbf-out-dir target/deploy
cargo test
```

Build first: the tests `include_bytes!` the `.so`. The code was written on Windows on Sep 16 and has
never been compiled, so expect at least one round of fixes.

What was added on top of the class AMM (needed for the README):

- `Config` gained `treasury` and `protocol_fee`. `initialize` checks `fee + protocol_fee <= 10_000` and
  `mint_x != mint_y`; new error `InvalidTreasury`.
- `.unwrap()` on curve results in deposit, withdraw and swap became `.map_err(AmmError::from)?`. The
  first deposit requires both sides > 0.
- `swap`: lock check; `treasury` pinned with `address = config.treasury`; `treasury_x` / `treasury_y`
  ATAs with `init_if_needed`; the protocol cut is taken from the input and sent to the treasury, the rest
  goes through the curve.
- Library bug workaround: `constant-product-curve`'s `swap` skips the fee for `LiquidityPair::Y` (around
  lib.rs L299-303 it uses `a` instead of `a2`), so the program always swaps as `LiquidityPair::X` with the
  reserves passed in input order.
- New `update.rs`: `lock` / `unlock`, gated by the `Option<Pubkey>` authority.
- Tests rewritten: a `Pool` struct of addresses, side-effect-free instruction builders, helpers
  `fund_user`, `balance`, `lp_supply`, `read_config`, `assert_amm_error`, `expected_swap`. 17 tests
  covering happy paths with exact balances and a failure case per error.

When green: screenshot to `Assignment_3_AMM/screenshots/cargo_test.png`, then write
`Assignment_3_AMM/README.md` in the style of `Assignement_2_escrow/escrow/README.md` (intro, instructions
table, accounts and PDAs, fee and treasury design, build and test, proof screenshot, tests table, notes
and gotchas). The optional extensions (CPMM without the library, downtime mitigation) were not done.

## Backlog: A4 Token-2022, `Assignement_4_Token22CTs`

Scope, as captured in Rohit's repo (`rohit0110/Turbin3Assignments`, `assignment4/README.md`): a
Token-2022 stablecoin with transfer fee, metadata pointer, default-frozen accounts, mint close authority,
fee-aware transfers read through `StateWithExtensions`, a KYC thaw, and a confidential-transfer mint
plus lifecycle. Ask Josh before starting, and whether he wants to write it himself or be guided.

## Not on this machine

The capstone drafts (LOI, review notes, a plan file) are untracked on the Windows machine. Don't
recreate them. Teammate repos: `rohit0110/heirloom`, `rohit0110/Turbin3Assignments`.
