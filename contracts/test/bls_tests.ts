import {ethers} from "ethers";
import {
    operatorsContractCode,
    BlsOperationsContractCode,
    deployBlsOperations
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

    let blsOperationsDeployedContract;

    beforeEach(async () => {
        console.log("---\n");
        blsOperationsDeployedContract = await deployBlsOperations(wallet, BlsOperationsContractCode);
    });

    it("Operations", async () => {
        // ETH deposit: Wrong tx value (msg.value < fee)
        console.log("\n - ETH deposit: Wrong tx value (msg.value < fee) started");
    });
});