import {ethers} from "ethers";
import {
    operatorsContractCode,
    blsVerifyRawTesterContractCode,
    deployBlsVerifyRawTester
} from "../src.ts/deploy";

import {expect, use, assert} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther, hexlify, BigNumber} from "ethers/utils";
import {
    hex_to_ascii
} from "./helpers"

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

describe("BLS Operations", function() {
    this.timeout(50000);

    let blsVerifyRawTesterDeployedContract;
    let message = '0x7b0a2020226f70656e223a207b0a20202020227072696365223a2039353931372c0a202020202274696d65223a207b0a20202020202022756e6978223a20313438333134323430302c0a2020202020202269736f223a2022323031362d31322d33315430303a30303a30302e3030305a220a202020207d0a20207d2c0a202022636c6f7365223a207b0a20202020227072696365223a2039363736302c0a202020202274696d65223a207b0a20202020202022756e6978223a20313438333232383830302c0a2020202020202269736f223a2022323031372d30312d30315430303a30303a30302e3030305a220a202020207d0a20207d2c0a2020226c6f6f6b7570223a207b0a20202020227072696365223a2039363736302c0a20202020226b223a20312c0a202020202274696d65223a207b0a20202020202022756e6978223a20313438333232383830302c0a2020202020202269736f223a2022323031372d30312d30315430303a30303a30302e3030305a220a202020207d0a20207d0a7d0a6578616d706c652e636f6d2f6170692f31';

    beforeEach(async () => {
        console.log("---\n");
        blsVerifyRawTesterDeployedContract = await deployBlsVerifyRawTester(wallet, blsVerifyRawTesterContractCode);
    });

    it("Raw verify", async () => {
        console.log("\n - Raw verify started");
        let sigX = '11181692345848957662074290878138344227085597134981019040735323471731897153462';
        let sigY = '6479746447046570360435714249272776082787932146211764251347798668447381926167';
        let result = await blsVerifyRawTesterDeployedContract.testVerify(message, sigX, sigY);
        expect(result).to.eq(true);
        console.log("\n + Raw verify finished");
    });
});