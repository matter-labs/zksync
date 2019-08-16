import {ethers} from "ethers";
import {deployContract} from "ethereum-waffle";
import {bigNumberify} from "ethers/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);


async function main() {
    const verifierContractCode = require('../build/VerifyTest.json');
    let contract = await deployContract(wallet, verifierContractCode, [], {
        gasLimit: 8000000,
    });
    console.log("contract deployed: ",contract.address);

    const blockProof = ["0x9d2a07078f7827da52e8b59196a59143c23d1ce46eeb732c5a64f9c8f70022a",
        "0x15af07e1dbe5da4ba159278d37f5b3b550c95a98a5ecf4a1830170dbdc57aa01",
        "0xb1a4527cf195934249d1de157f2621e5e6098c27c23b22aa55b5ebbf55274a1",
        "0x11f12c2f7149c50761b33c0ce1632f03c06679a02fb180f6bbf3cd5b9825ff91",
        "0x5cb55c231b3ad6982f4846f88ac92fb43feb7e34d3261a4207621221d2e31e7",
        "0x104955fe794e0630b65efd94dccd91925ce5573a692d597402889e069e7e1150",
        "0x2051bb039d373074465e68da4770a4d4c9d4ea80e7104cea7daf8063db3d6417",
        "0x2ad34d0969f70b9672cb5da95a0a58a0990bfeb07700d84abfef99c595e990c9"];
    const commitment = "0x0992e4542bf09cbecd3f8b433b8a0475b85f3836a2b766a0a2081cf4f16715c3";
    let tx = await contract.verifyProof(commitment, blockProof, { gasLimit: 1000000});
    console.log("tx: ", tx);
    let receipt = await tx.wait();
    console.log("receipt: ",receipt);

}

main();