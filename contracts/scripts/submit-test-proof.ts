import {deployContract} from "ethereum-waffle";
import {ethers} from "ethers";
import {Deployer} from "../src.ts/deploy";
import {bigNumberify} from "ethers/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);

const MAX_CONTRACT_SIZE_BYTES = 24576;
async function main() {
    const deployer = new Deployer({deployWallet: wallet});

    const pubInput = ['0x397c5d1d7e63f4532f20d6f6b4ef064913c2a34ad264ebd07a0c499653e18a'];
    const proof = ['0x21be048baf97bdf473fece1774bf20d7beea80614f81b4f9d6e7bf22fa5dc48a',
        '0x193dde6a46a951e0bee6501428604eb52015c57964286dcb1e4fd028e3fa839f',
        '0x2c5ad19f7deb008d963b598728ba603ac659d942f2f72bb6471a04862604996e',
        '0x13489b4262b85fb845ec1b3dbd92aefab1654bf0bfa9140856b9815e345b488e',
        '0x225dd956e1e09f524408007723c18f7f4a2aa8587758aa51ae18bb4a94d2ba26',
        '0x124ae981cbdc96dbddc4e0e54f5ea6e62fcef5f208db0711a98c27db29e2f60b',
        '0x1b58a717b0fe5a330273cf7c73772d3b7f2cb8aaf8d24e58f13e91e86d36cd86',
        '0x19d5c9135bdb3b57b4e5075b271cd255111e846a2fd0b57c6455de1806714e8a',
        '0x7a0bf6c16f9c950576469744df6faa982a44a4f00426271b3141fc00934596',
        '0x1156f8700b4cd5172c62089862025ae6d3e41d0daa251006f53b7a7fe894307f',
        '0x173f044962e4680ed8a8eb077a2904512ee852455007ddc5c3741a30a0d86d0b',
        '0x104c42a02790e3e514c95dbf6f9fc3f2f61168de897e12a5ffb2baed2cfe4e63',
        '0x0c4b8df854eaa361de37d19cdc674dff549f789ad843161a732d74c3b850b56c',
        '0x0fecfe91b36131effadc8a9652b5fdd596519de7005ece0a4e8b4f528550fc9d',
        '0x05066a5ac7fd240443e14eb533ec2474bd29087d83493a410336185a4054773c',
        '0x16dfec216639863eba3b96ed3bb0b8fe7bd7bfb4f98310e9f95f8881a01190cb',
        '0x020c24d675b0d7129b00dd301950fe2f49b3b4e698b0f07c2a3f31bd56ee0f05',
        '0x06c6e5d15d5769674d2367a14718b9d67968bbea559175e48724673cbdceabc7',
        '0x13b9c667b500d035dbb133da529e8f1261010858fe0c2b8e2cffcc0d1fd8401a',
        '0x0a00f2b910c3fbca1e4a6fc481a8b64330b3a7c06590f50b345b8c75371c076f',
        '0x045ceb28e15f2d300562e001169480028a18743d9d8120b319ce29266d179053',
        '0x2ba3995d314d4805d782e85a1048dda47f6e4cc39de78ca809165dd611cc68b2',
        '0x16c5761482ea62f9b6543d5153bda89a38fd63e4729115dab68acf74358abc4f',
        '0x0e377ce58dd2e2f4f8c14514b476c9f8841dfd2e1cd20c446f1494ba375cc923',
        '0x15330e60a95372ea70009e55776927035e1c90ddd6928e84bdae152e0bfb88cf',
        '0x0ff4f39928fb5517a484e88c92cec1770613e584e45cd9b2287b5961d55c898a',
        '0x2bbb1b81a72e88553b9def920054ba7224ee133796af5006a001872f925a5c26',
        '0x2814edc80c64f4a265faaf78bb0d57e6da777a4cef636601dabdd6a8bc530c20',
        '0x2f025a91770f829611d638c6edd922c8e40b76baeff8f19cd0e6e945accb7709',
        '0x27fb44e1c8d7f23c499b650d0845f46340697fa4ac3d8840e1328e1149487950',
        '0x2af70d65d06e75402369222b14e3ea95cde2ab1a748be9d4bd28fb80a8630a7c',
        '0x0256e87f41d05afa4284e98ed7b1cc7d6b1a2d672dc46f55865cc0eb1864d00a',
        '0x1fef5ac730ce2dedec7c1a857382549033fc520ec5c716eb59479c6d2d07f86c',
        '0x198e8a63e6c775a79907823a7e71bf4e8aa00c41752c5f7003a9184e8a879669'];
    const subproofLimbs = ['0x0efd6c2e865d3920e9',
        '0x0fde563c721dea255b',
        '0x0c49f4fbfa342e7253',
        '0x02a321a9ba1d79',
        '0x08af6e14a05dc07dbb',
        '0x0b7d14f64a884ad56f',
        '0x017c86d029c9de41fc',
        '0x01756198f2fb69',
        '0x3cbf4635cfe5d835',
        '0x0cf1059d89e8da1dc0',
        '0x0cf09efb5086e17969',
        '0x02690e8706ce8f',
        '0x04aaa871547e02e65a',
        '0x0546f6c9a5a56680cf',
        '0x032f9c2b3308950d87',
        '0x217156550459'];
    // const verifier = deployer.verifierContract(wallet);
    // const zkSyncInputs = ["0xe9b896cf1d55b50644031165bb054ce7b43b84f5a2d479963b1c66ab51427f2"];
    // const result = await verifier.verifyMultiblockProof(pubInput, proof, [6], zkSyncInputs, subproofLimbs);
    // console.log(await result.wait());

    const zksync = deployer.zkSyncContract(wallet);
    console.log(await zksync.blocks(1))
    return;
    const tx = await zksync.verifyBlocks(
        1,
        1,
        pubInput,
           proof,
            subproofLimbs,
        [ '0x' ],
        {gasLimit: bigNumberify("1000000")}
    );
    await tx.wait();
    console.log(tx.hash);
}

main();
