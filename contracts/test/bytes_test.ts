import { ethers } from "ethers";

import { expect, use } from "chai";
import { solidity, deployContract } from "ethereum-waffle";
import { bigNumberify, parseEther, hexlify, formatEther } from "ethers/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet: any = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

use(solidity);

export async function deployBytesTestContract() {
    try {
        return await deployContract(wallet, require('../build/VerifierTest'), [], {
            gasLimit: 2000000,
        });
    } catch (err) {
        console.log("BytesTest deploy error:" + err);
    }
}

describe("Bytes unit test", function () {
    this.timeout(50000);

    let bytesTestContract: any;
    beforeEach(async () => {
        console.log("---\n");
        bytesTestContract = await deployBytesTestContract();
    });

    it("should bla-bla", async () => {
        console.log("\n - Bytes test started");

        // // Commit block with eth deposit
        // const depositBlockPublicData = createDepositPublicData(0, hexlify(depositAmount), franklinAddress);
        // const feeAccount = 22;
        // const root = "0000000000000000000000000000000000000000000000000000000000000000";
        // let commitment = "0xc456a531f6b89e6c0bf3a381b03961725895447203ec77cb0a2afd95e78217dd";
        // await postBlockCommit(
        //     provider,
        //     wallet,
        //     franklinDeployedContract,
        //     1,
        //     feeAccount,
        //     root,
        //     depositBlockPublicData,
        //     1,
        //     1,
        //     commitment,
        //     null,
        // );

        // expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);

        console.log(" + Bytes test passed")
    });

});
