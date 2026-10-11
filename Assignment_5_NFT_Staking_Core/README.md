# nft-staking-core

Week 5 assignment for Turbin3 (Q3 2026 cohort). NFT staking for Metaplex Core assets in Anchor, built on top of the class program ([AndreiaCanadas/nft-staking-core](https://github.com/AndreiaCanadas/nft-staking-core)).

The base program could already stake a Core NFT (freeze it in the owner's wallet) and unstake it for rewards. This version adds the three Task 1 features:

- `claim_rewards`: collect rewards while the NFT stays staked and frozen
- `burn_staked_nft`: burn a staked NFT through the BurnDelegate plugin for a one-time bonus
- a `total_staked` counter stored as an Attribute on the collection account itself

Staking is non-custodial. The NFT never leaves the owner's wallet. The staking state lives on the asset in an Attributes plugin, the lock is the FreezeDelegate plugin, and the program acts through a PDA that is the collection's update authority.

Task 2 (the optional Oracle plugin) is not implemented.

## The instructions

| Instruction | What it does |
|---|---|
| `create_collection` | Creates the collection with a PDA as update authority, and gives it an Attributes plugin starting at `total_staked = 0` |
| `mint_nft` | Mints a Core NFT into the collection |
| `initialize_config` | Creates the `Config` PDA and the rewards mint. Takes `points_per_stake`, `freeze_period` (days) and `burn_bonus` |
| `stake` | Writes `staked`, `staked_at` and `last_claimed_at` on the NFT, adds BurnDelegate and FreezeDelegate if they are missing, freezes it, increments `total_staked` |
| `claim_rewards` | Pays every full day since `last_claimed_at`, moves `last_claimed_at` forward, leaves the NFT staked and frozen |
| `unstake` | Checks the freeze period, pays the days not claimed yet, resets the attributes, thaws the NFT, decrements `total_staked` |
| `burn_staked_nft` | Pays the unclaimed days plus `burn_bonus`, thaws the NFT, burns it as the BurnDelegate, decrements `total_staked` |

## State

On each NFT, in its Attributes plugin:

| Key | Value |
|---|---|
| `staked` | `"true"` or `"false"` |
| `staked_at` | Unix time of the stake. Only used for the freeze period |
| `last_claimed_at` | Unix time rewards are counted from |

On the collection, in its Attributes plugin: `total_staked`.

The `Config` PDA, one per collection:

```rust
pub struct Config {
    pub points_per_stake: u32,
    pub freeze_period: u8,
    pub burn_bonus: u64,
    pub rewards_bump: u8,
    pub config_bump: u8,
}
```

| Account | Seeds |
|---|---|
| Update authority | `["update_authority", collection]` |
| Config | `["config", collection]` |
| Rewards mint | `["rewards", config]` |

## How the three features work

**Claim without unstaking.** The base program paid rewards from `staked_at` on unstake. Once you can claim halfway through, that would pay the claimed days twice, so there is now a second timestamp. `staked_at` stays fixed for the freeze period check, and `last_claimed_at` is where rewards are counted from. A claim moves `last_claimed_at` forward by whole days only (`days * 86400`), not to the current time, so a partial day is not thrown away. `unstake` and `burn_staked_nft` both pay from `last_claimed_at`.

**Burn-to-earn.** BurnDelegate is an owner-managed plugin, so it can only be added with the owner's signature. `stake` is the one place the owner signs, so that is where it gets added, with the update authority PDA as the delegate. Core will not burn a frozen asset, so `burn_staked_nft` thaws it first and then burns it, both in the same instruction. The reward is the unclaimed days plus `config.burn_bonus`.

**Collection stats.** `total_staked` is an Attributes plugin on the collection, managed by the collection's update authority (the PDA). It is created at `create_collection`, and `stake`, `unstake` and `burn_staked_nft` rewrite it through `UpdateCollectionPluginV1`. For collections made before this change, the first stake adds the plugin. The value is a string, so it is parsed, changed with checked math and written back.

## Build and test

This one is on Anchor 0.32.1, the version the class program uses, not the 1.x my other assignments use. Tests run on [Surfpool](https://surfpool.run), which forks mainnet, so Metaplex Core is available without cloning it in `Anchor.toml`. It needs an internet connection.

```bash
avm use 0.32.1
yarn install
anchor keys sync
anchor build
anchor test
```

The tests jump the clock with Surfpool's `surfnet_timeTravel`. Time only moves forward, so the tests depend on running in order.

### Proof

![anchor test](screenshots/anchor_test.png)

## The tests

| Test | Covers |
|---|---|
| creates the collection with total_staked = 0 | The collection starts with the counter attribute |
| mints two NFTs | Setup |
| initializes the config with a burn bonus | `burn_bonus` is stored |
| stakes both NFTs and counts them on the collection | `staked = true`, frozen, BurnDelegate present, `total_staked = 2` |
| rejects staking the same NFT twice | `AlreadyStaked` |
| rejects unstaking before the freeze period | `FreezePeriodNotElapsed` |
| rejects a claim before a full day has passed | `NothingToClaim` |
| claims 8 days of rewards and keeps the NFT staked and frozen | Exact reward amount, still frozen, `last_claimed_at = staked_at + 8 days` |
| rejects a second claim on the same day | `NothingToClaim` |
| rejects a claim from a wallet that does not own the NFT | `InvalidOwner` |
| unstakes and only pays the days after the last claim | Total is 10 days of rewards, not 18. Thawed, `total_staked = 1` |
| rejects claiming on an unstaked NFT | `NotStaked` |
| rejects burning an unstaked NFT | `NotStaked` |
| burns a staked NFT for its rewards plus the bonus | 10 days plus `burn_bonus` paid, asset gone, `total_staked = 0` |

The time jumps land half a day past the target (8.5 and 10.5 days after the stake), so the whole-day math is not thrown off by the few seconds between transactions.

## Notes and gotchas

- **Frozen assets cannot be burned.** FreezeDelegate blocks burns as well as transfers, so a BurnDelegate on its own is not enough. The thaw has to come first.
- **The PDA keeps its BurnDelegate after unstake.** The plugin stays on the NFT, so in theory the program could still burn it. `burn_staked_nft` refuses anything that is not `staked = true`, and no other instruction burns, so that power is never used.
- **Compute units.** `stake` now makes up to four CPIs into Core (attributes, BurnDelegate, FreezeDelegate, collection counter). The tests set a 400k CU limit on every staking instruction rather than relying on the 200k default.
- **`fetchAssetV1`, not `fetchAsset`, in the tests.** `fetchAsset` merges the collection's plugins into the asset it returns, so the collection's `total_staked` attribute would show up on an NFT that has no attributes of its own.
- **`claim_rewards` ignores the freeze period.** The freeze period only limits unstaking. Rewards can be collected after any full day.

## Reference docs

- [Metaplex Core: Using Core in Anchor](https://developers.metaplex.com/core/using-core-in-anchor)
- [Metaplex Core: Freeze Delegate](https://developers.metaplex.com/core/plugins/freeze-delegate), [Burn Delegate](https://developers.metaplex.com/core/plugins/burn-delegate), [Attributes](https://developers.metaplex.com/core/plugins/attribute)
- [Surfpool time travel](https://solana.com/docs/tools/surfpool/sdk/time-travel)
