import {
  appendTransactionMessageInstruction,
  appendTransactionMessageInstructions,
  assertIsTransactionMessageWithBlockhashLifetime,
  assertIsTransactionWithBlockhashLifetime,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  generateKeyPairSigner,
  getSignatureFromTransaction,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from "@solana/kit";
import {
  getInitializeMintInstruction,
  getMintSize,
  TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import { getCreateAccountInstruction } from "@solana-program/system";

//import your wallet
import wallet from "../../devnet-wallet.json";

const rpc = createSolanaRpc("https://api.devnet.solana.com");

const rpcSubscriptions = createSolanaRpcSubscriptions(
  "wss://api.devnet.solana.com",
);

(async () => {
  try {
    // create a signer from ur wallet
    const signer = await createKeyPairSignerFromBytes(new Uint8Array(wallet));

    //gen a new mint signer for address
    const mint = await generateKeyPairSigner();
    const mintSize = BigInt(getMintSize());

    //getting the space 
    const space = BigInt(getMintSize());

    //now we are getting the min balance for the rent exemption
    const rent = await rpc.getMinimumBalanceForRentExemption(mintSize).send();

    //FLOW
    //creating a msg --(listing all the instructions that need to go in a txn--> get the block hash --> sign --> send txn --> cofirm

    //getting the latest blockhash
    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    //send and confim func
    const sendAndConfirm = sendAndConfirmTransactionFactory(
      {rpc, rpcSubscriptions}
    );

    const msg = createTransactionMessage({version: 0})

    const msgWithPayer = setTransactionMessageFeePayerSigner(signer, msg);

    const msgWithLifetime = setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, msgWithPayer);

    const txnMessage = appendTransactionMessageInstructions(
      [
        getCreateAccountInstruction({
          payer: signer,
          newAccount: mint,
          lamports: rent,
          space: mintSize,
          programAddress: TOKEN_PROGRAM_ADDRESS,
        }),
        getInitializeMintInstruction({
          mint: mint.address,
          decimals: 0,
          mintAuthority: signer.address,
        }),
      ],
      msgWithLifetime,  //txn message that it is being appended to
    );

     const signedTx = await signTransactionMessageWithSigners(txnMessage);
    assertIsTransactionWithBlockhashLifetime(signedTx);

    const signature = await sendAndConfirm(signedTx, {commitment: "confirmed"});
    console.log(`Mint Address: ${mint.address}, Minted token with signature: ${signature}`);
  } catch (error) {
    console.log(error);
  }
})();
