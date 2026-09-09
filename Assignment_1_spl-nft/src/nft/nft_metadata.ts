import {
  createSignerFromKeypair,
  signerIdentity,
} from "@metaplex-foundation/umi";
import wallet from "../../devnet-wallet.json";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { irysUploader } from "@metaplex-foundation/umi-uploader-irys";

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
    //change the image uri to your image uri obtained from nft_image.ts
    const image =
      "https://gateway.irys.xyz/8vexLr2bYE1HNt4YfieYQXwhkJFX7HqX8U978dK1diDE";

    //json scheme : https://www.metaplex.com/docs/smart-contracts/core/json-schema
    
    //change the metadata from the one recived
    const metadata = {
      name: "AI Layer 2",
      symbol: "AIL2",
      description: "The ultimate hacker on a toy laptop. Built different.",
      image,
      attributes: [
        { trait_type: "Background", value: "Matrix" },
        { trait_type: "Vibe", value: "Hacker" },
        { trait_type: "Accessory", value: "Pink Shades" },
        { trait_type: "Device", value: "Toy Laptop" },
      ],
      properties: {
        files: [
          {
            type: "image/jpeg",
            uri: image,
          },
        ],
      },
    };

    const myUri = await umi.uploader.uploadJson(metadata);
    
    console.log(`metadata uri: ${myUri} `);
  
  } catch (error) {
    console.log("error", error);
  }
})();
