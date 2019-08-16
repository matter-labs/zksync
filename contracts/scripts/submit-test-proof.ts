import {ethers} from "ethers";
import {deployContract} from "ethereum-waffle";
import {bigNumberify} from "ethers/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);


async function main() {
    const vkContractCode = require('../build/VerificationKey');
    const verifierContractCode = require('../build/Verifier');
    let verifier = await deployContract(wallet, verifierContractCode, [], {
        gasLimit: 1000000,
    });
    let vk = await deployContract(wallet, vkContractCode, [], {
        gasLimit: 1000000,
    });
    const verifierTesterCode = require('../build/VerifyTest.json');
    let contract = await deployContract(wallet, verifierTesterCode, [verifier.address, vk.address], {
        gasLimit: 8000000,
    });
    console.log("contract deployed: ",contract.address);

    const blockProof =["0x15e066ca37c11de0876fa36785f00b68dac39c8bbc2e34e47b965dacba4074dc", "0x9f9c4871f9f2a11ff1b216275059b0a7c22e4e080aa69da3d8739817bda6f67", "0x10404b29f3c518b719e42b745dea2018902aec19270fee025a385d4fa45ee2bd", "0x11eb9871e1282c4ca28f372337d91b2f5f17ec5faf7455bef099dcc0cd86bf89", "0xed955100328e528eba03f8efc2e9796b427939c731433ff5dbe46e5efa4f43", "0xc765933d79aae1a0eefa8316a6ef829b80e06882fd895c46977aa1cd04bb0b1", "0x1671b2983be650235a08d4c22a8022451421401755ac25affb3ed8ef29385b7b", "0x133ca618e90c85db76b34e4b12176a1599c35b77544b5479c3505415c7d7838b"];
    const commitment = "0x087bbd4a201ac4bdac24a2d08b4bff09f1ad08c77744267ea4767b120f39c253";
    let tx = await contract.verifyProof(commitment, blockProof, { gasLimit: 1000000});
    console.log("tx: ", tx);
    let receipt = await tx.wait();
    console.log("receipt: ",receipt);

}

main();