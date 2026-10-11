import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { NftStakingCore } from "../target/types/nft_staking_core";
import { ComputeBudgetProgram, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { MPL_CORE_PROGRAM_ID, fetchAssetV1, fetchCollectionV1 } from "@metaplex-foundation/mpl-core";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { publicKey } from "@metaplex-foundation/umi";
import { ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { expect } from "chai";

const DAY = 86400;
const POINTS_PER_DAY = 10_000_000;
const FREEZE_PERIOD_IN_DAYS = 7;
const BURN_BONUS = 1_000_000_000;

type AttributeList = { key: string; value: string }[] | undefined;

describe("nft-staking-core", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.nftStakingCore as Program<NftStakingCore>;
  const umi = createUmi(provider.connection.rpcEndpoint);

  const user = provider.wallet.publicKey;
  const stranger = Keypair.generate();
  const collection = Keypair.generate();
  const nft1 = Keypair.generate();
  const nft2 = Keypair.generate();

  const [updateAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("update_authority"), collection.publicKey.toBuffer()],
    program.programId
  );
  const [config] = PublicKey.findProgramAddressSync(
    [Buffer.from("config"), collection.publicKey.toBuffer()],
    program.programId
  );
  const [rewardsMint] = PublicKey.findProgramAddressSync(
    [Buffer.from("rewards"), config.toBuffer()],
    program.programId
  );

  const computeBudget = [ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })];
  let stakedAt = 0;

  const rewardsAta = (owner: PublicKey) =>
    getAssociatedTokenAddressSync(rewardsMint, owner, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);

  const stakeAccounts = (nft: PublicKey, owner: PublicKey = user) => ({
    user: owner,
    updateAuthority,
    config,
    nft,
    collection: collection.publicKey,
    mplCoreProgram: MPL_CORE_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  });

  const rewardAccounts = (nft: PublicKey, owner: PublicKey = user) => ({
    ...stakeAccounts(nft, owner),
    rewardsMint,
    userRewardsAta: rewardsAta(owner),
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  });

  const attr = (list: AttributeList, key: string) => list?.find((a) => a.key === key)?.value;

  const fetchNft = (nft: PublicKey) =>
    fetchAssetV1(umi, publicKey(nft.toBase58()), { commitment: "confirmed" });

  const totalStaked = async () => {
    const c = await fetchCollectionV1(umi, publicKey(collection.publicKey.toBase58()), { commitment: "confirmed" });
    return attr(c.attributes?.attributeList, "total_staked");
  };

  const rewardBalance = async () =>
    Number((await provider.connection.getTokenAccountBalance(rewardsAta(user), "confirmed")).value.amount);

  async function expectError(call: Promise<unknown>, code: string) {
    try {
      await call;
    } catch (err: any) {
      const text = [err?.error?.errorCode?.code, err?.message, ...(err?.logs ?? [])].join(" ");
      expect(text).to.include(code);
      return;
    }
    expect.fail(`expected ${code}`);
  }

  async function travelTo(unixSeconds: number) {
    const res = await fetch(provider.connection.rpcEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "surfnet_timeTravel",
        params: [{ absoluteTimestamp: unixSeconds * 1000 }],
      }),
    });
    const json = (await res.json()) as { error?: unknown };
    if (json.error) {
      throw new Error(`Time travel failed: ${JSON.stringify(json.error)}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  it("creates the collection with total_staked = 0", async () => {
    await program.methods
      .createCollection("Test Collection", "https://example.com/collection")
      .accountsPartial({
        payer: user,
        collection: collection.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([collection])
      .rpc({ commitment: "confirmed" });

    expect(await totalStaked()).to.equal("0");
  });

  it("mints two NFTs", async () => {
    for (const nft of [nft1, nft2]) {
      await program.methods
        .mintNft("Test NFT", "https://example.com/nft")
        .accountsPartial({
          user,
          nft: nft.publicKey,
          collection: collection.publicKey,
          updateAuthority,
          systemProgram: SystemProgram.programId,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
        })
        .signers([nft])
        .rpc({ commitment: "confirmed" });
    }
  });

  it("initializes the config with a burn bonus", async () => {
    await program.methods
      .initializeConfig(POINTS_PER_DAY, FREEZE_PERIOD_IN_DAYS, new BN(BURN_BONUS))
      .accountsPartial({
        admin: user,
        collection: collection.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc({ commitment: "confirmed" });

    const cfg = await program.account.config.fetch(config, "confirmed");
    expect(cfg.burnBonus.toNumber()).to.equal(BURN_BONUS);
  });

  it("stakes both NFTs and counts them on the collection", async () => {
    for (const nft of [nft1, nft2]) {
      await program.methods
        .stake()
        .accountsPartial(stakeAccounts(nft.publicKey))
        .preInstructions(computeBudget)
        .rpc({ commitment: "confirmed" });
    }

    const asset = await fetchNft(nft1.publicKey);
    expect(attr(asset.attributes?.attributeList, "staked")).to.equal("true");
    expect(asset.freezeDelegate?.frozen).to.equal(true);
    expect(asset.burnDelegate).to.not.equal(undefined);
    stakedAt = Number(attr(asset.attributes?.attributeList, "staked_at"));
    expect(await totalStaked()).to.equal("2");
  });

  it("rejects staking the same NFT twice", async () => {
    await expectError(
      program.methods.stake().accountsPartial(stakeAccounts(nft1.publicKey)).preInstructions(computeBudget).rpc(),
      "AlreadyStaked"
    );
  });

  it("rejects unstaking before the freeze period", async () => {
    await expectError(
      program.methods.unstake().accountsPartial(rewardAccounts(nft2.publicKey)).preInstructions(computeBudget).rpc(),
      "FreezePeriodNotElapsed"
    );
  });

  it("rejects a claim before a full day has passed", async () => {
    await expectError(
      program.methods.claimRewards().accountsPartial(rewardAccounts(nft1.publicKey)).preInstructions(computeBudget).rpc(),
      "NothingToClaim"
    );
  });

  it("claims 8 days of rewards and keeps the NFT staked and frozen", async () => {
    await travelTo(stakedAt + 8 * DAY + DAY / 2);

    await program.methods
      .claimRewards()
      .accountsPartial(rewardAccounts(nft1.publicKey))
      .preInstructions(computeBudget)
      .rpc({ commitment: "confirmed" });

    expect(await rewardBalance()).to.equal(8 * POINTS_PER_DAY);
    const asset = await fetchNft(nft1.publicKey);
    expect(attr(asset.attributes?.attributeList, "staked")).to.equal("true");
    expect(asset.freezeDelegate?.frozen).to.equal(true);
    expect(Number(attr(asset.attributes?.attributeList, "last_claimed_at"))).to.equal(stakedAt + 8 * DAY);
    expect(await totalStaked()).to.equal("2");
  });

  it("rejects a second claim on the same day", async () => {
    await expectError(
      program.methods.claimRewards().accountsPartial(rewardAccounts(nft1.publicKey)).preInstructions(computeBudget).rpc(),
      "NothingToClaim"
    );
  });

  it("rejects a claim from a wallet that does not own the NFT", async () => {
    await provider.sendAndConfirm(
      new Transaction().add(
        SystemProgram.transfer({ fromPubkey: user, toPubkey: stranger.publicKey, lamports: LAMPORTS_PER_SOL })
      )
    );
    await expectError(
      program.methods
        .claimRewards()
        .accountsPartial(rewardAccounts(nft2.publicKey, stranger.publicKey))
        .preInstructions(computeBudget)
        .signers([stranger])
        .rpc(),
      "InvalidOwner"
    );
  });

  it("unstakes and only pays the days after the last claim", async () => {
    await travelTo(stakedAt + 10 * DAY + DAY / 2);

    await program.methods
      .unstake()
      .accountsPartial(rewardAccounts(nft1.publicKey))
      .preInstructions(computeBudget)
      .rpc({ commitment: "confirmed" });

    expect(await rewardBalance()).to.equal(10 * POINTS_PER_DAY);
    const asset = await fetchNft(nft1.publicKey);
    expect(attr(asset.attributes?.attributeList, "staked")).to.equal("false");
    expect(asset.freezeDelegate?.frozen).to.equal(false);
    expect(await totalStaked()).to.equal("1");
  });

  it("rejects claiming on an unstaked NFT", async () => {
    await expectError(
      program.methods.claimRewards().accountsPartial(rewardAccounts(nft1.publicKey)).preInstructions(computeBudget).rpc(),
      "NotStaked"
    );
  });

  it("rejects burning an unstaked NFT", async () => {
    await expectError(
      program.methods.burnStakedNft().accountsPartial(rewardAccounts(nft1.publicKey)).preInstructions(computeBudget).rpc(),
      "NotStaked"
    );
  });

  it("burns a staked NFT for its rewards plus the bonus", async () => {
    const before = await rewardBalance();

    await program.methods
      .burnStakedNft()
      .accountsPartial(rewardAccounts(nft2.publicKey))
      .preInstructions(computeBudget)
      .rpc({ commitment: "confirmed" });

    expect((await rewardBalance()) - before).to.equal(10 * POINTS_PER_DAY + BURN_BONUS);
    const info = await provider.connection.getAccountInfo(nft2.publicKey, "confirmed");
    expect(info === null || info.data.length <= 1).to.equal(true);
    expect(await totalStaked()).to.equal("0");
  });
});
