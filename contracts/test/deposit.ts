import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin} from "../src.ts/deploy";

import {expect, use} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther} from "ethers/utils";

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const franklinAddress = "010203040506070809101112131415161718192021222334252627";

describe("INTEGRATION: Deposit", function() {
    this.timeout(20000);

    let franklinDeployedContract;
    let erc20DeployedToken;

    before(async () => {
        franklinDeployedContract = await deployFranklin(wallet);
        erc20DeployedToken = await addTestERC20Token(wallet, franklinDeployedContract);
    });

    it("Ether deposit", async () => {
        const addressBytes = Buffer.from(franklinAddress, "hex");
        const tx = await franklinDeployedContract.depositETH(addressBytes, {value: parseEther("0.3")});
        const receipt = await tx.wait();
        const event = receipt.events.pop().args;

        expect(event.owner).equal(wallet.address);
        expect(event.tokenId).equal(0);
        expect(event.amount).equal(parseEther("0.3"));
        expect(event.franklinAddress).equal("0x" + franklinAddress);
    });

    it("ERC20 deposit", async () => {
        const depositValue = bigNumberify("78");
        await erc20DeployedToken.approve(franklinDeployedContract.address, depositValue);

        const addressBytes = Buffer.from(franklinAddress, "hex");
        const tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, depositValue, addressBytes,
            {gasLimit: bigNumberify("150000")});
        const receipt = await tx.wait();
        const event = receipt.events.pop().args;

        expect(event.owner).equal(wallet.address);
        expect(event.tokenId).equal(1);
        expect(event.amount).equal(depositValue);
        expect(event.franklinAddress).equal("0x" + franklinAddress);
    });

});
