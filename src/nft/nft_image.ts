import dotenv from "dotenv";
dotenv.config({ path: "e.env" });
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import {
  createGenericFile,
  createSignerFromKeypair,
  signerIdentity,
} from "@metaplex-foundation/umi";
import { irysUploader } from "@metaplex-foundation/umi-uploader-irys";
import { readFile } from "fs/promises";

import wallet from "../../devnet-wallet.json";

const umi = createUmi(
  process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com",
);

const keypair = umi.eddsa.createKeypairFromSecretKey(new Uint8Array(wallet));
const signer = createSignerFromKeypair(umi, keypair);

umi.use(
  irysUploader({
    address: "https://devnet.irys.xyz/",
  }),
);

umi.use(signerIdentity(signer));

(async () => {
  try {
    //chanege image path to your image path
    const imagePath = process.env.IMAGE_PATH;
    if (!imagePath) throw new Error("IMAGE_PATH not set in e.env");
    const image = await readFile(imagePath);

    //change the image name and mime type
    const file = createGenericFile(image, "generug.png", { contentType: "image/png" });

    const [myUri] = await umi.uploader.upload([file]);
     
    console.log("Your image URI: ", myUri);
  
  } catch (error) {
    console.log(error);
  }
})();
