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

    const blockProof = ["0x27e0b14ed7cbe474119bd74a61f2cfdf5fd1b40d79d27f8b160dec8e2504e280", "0x2f2de2636486efd4cdec44553ae718e3b882753c997c27fef40b500d50a6f8bf", "0x19163a83c4bdd7c0ea3327f7c94a624647953b633e19d1e43ef441b9d4b379a7", "0x2016902f52244d9659a64cce8c20c4ef9cba855047ed28e305933de773bfbc11", "0x256d6932ae135872e9b348348279d84d818a04d37144dbf995997af555b3cc4c", "0x670b1b6141ae1e0459390bbd5d336814ec903475c2a557d9285f081912ca65e", "0x6c72c9f3d247c8ff56af04bebdce11b59b622d131045c8a7ff985803f28ce01", "0x956da144c6eb7443372ba19754882a8abc59580c77070047c4ac26bfd8c68bd"];
    const commitment = "0x003efee8397082a496c6e7b8c18197e9a97d1cd416993002d8b2b289d1f91d33";
    let tx = await contract.verifyProof(commitment, blockProof, { gasLimit: 1000000});
    console.log("tx: ", tx);
    let receipt = await tx.wait();
    console.log("receipt: ",receipt);

}

main();