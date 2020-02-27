const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { wallet, deployTestContract, getCallRevertReason } = require("./common")
const {ethers } = require("ethers");
const zksync = require("zksync");


describe("ZK Sync signature verification unit tests", function () {
    this.timeout(50000);

    let testContract;
    let randomWallet = ethers.Wallet.createRandom();
    before(async () => {
        testContract = await deployContract(wallet, require('../../build/ZKSyncUnitTest'), [], {
            gasLimit: 6000000,
        });
    });

    it("signature verification success", async () => {
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {revertReason, result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), nonce, randomWallet.address));
        expect(result).eq(true);
    });

    it("signature verification incorrect nonce", async () => {
        const incorrectNonce = 0x11223345;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), incorrectNonce, randomWallet.address));
        expect(result).eq(false);
    });

    it("signature verification incorrect pubkey hash", async () => {
        const incorrectPubkeyHash = "sync:aaaafefefefefefefefefefefefefefefefefefe";
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, incorrectPubkeyHash.replace("sync:", "0x"), nonce, randomWallet.address));
        expect(result).eq(false);
    });

    it("signature verification incorrect signer", async () => {
        const incorrectSignerAddress = wallet.address;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), nonce, incorrectSignerAddress));
        expect(result).eq(false);
    });

});
//
// describe("ZK Sync deposit unit tests", function () {
//     this.timeout(50000);
//
//     let zksyncContract;
//     let tokenContract;
//     before(async () => {
//         const verifierDeployedContract = await deployVerifier(wallet, verifierTestContractCode, []);
//         const governanceDeployedContract = await deployGovernance(wallet, governanceTestContractCode, [wallet.address]);
//         zksyncContract = await deployFranklin(
//             wallet,
//             franklinTestContractCode,
//             [
//                 governanceDeployedContract.address,
//                 verifierDeployedContract.address,
//                 wallet.address,
//                 ethers.constants.HashZero,
//             ],
//         );
//         await governanceDeployedContract.setValidator(wallet.address, true);
//         tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
//         await mintTestERC20Token(wallet, tokenContract);
//     });
//
//     it("signature verification success", async () => {
//
//     });
//
// });
