import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, deployPriorityQueue} from "../src.ts/deploy";

import {expect, use} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther} from "ethers/utils";
import {createDepositPublicData, createPartialExitPublicData} from "./helpers"

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "010203040506070809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("INTEGRATION: PriorityQueue", function() {
    this.timeout(30000);

    let franklinDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        franklinDeployedContract = await deployFranklin(wallet);
        erc20DeployedToken = await addTestERC20Token(wallet, franklinDeployedContract);
        await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("Ether deposit, deposit request, commit, exit request, commit, withdraw", async () => {
        // Deposit eth
        const depositValue = parseEther("0.3");
        const depositFee = parseEther("0.01");
        const tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: depositValue});
        const receipt = await tx.wait();
        const event = receipt.events.pop().args;

        expect(event.owner).equal(wallet.address);
        expect(event.tokenId).equal(0);
        expect(event.amount).equal(depositValue);
        expect(event.franklinAddress).equal("0x" + franklinAddress);

        expect((await franklinDeployedContract.balances(wallet.address, 0)).balance).equal(depositValue);
        expect(await franklinDeployedContract.depositWasDone(wallet.address)).equal(true);
        expect(await franklinDeployedContract.depositFranklinToETH(franklinAddressBinary)).equal(wallet.address);
    }
}
