import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin} from "../src.ts/deploy";

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

describe("INTEGRATION: Deposit", function() {
    this.timeout(30000);

    let franklinDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        franklinDeployedContract = await deployFranklin(wallet);
        erc20DeployedToken = await addTestERC20Token(wallet, franklinDeployedContract);
        // Make sure that exit wallet can execute transactions.
        await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("Ether deposit, commit, withdraw, verify", async () => {

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

        // Commit block with eth deposit
        const depositBlockPublicData = createDepositPublicData(0, depositValue.sub(depositFee), depositFee, franklinAddress);
        const commitTx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        await commitTx.wait();
        expect((await franklinDeployedContract.balances(wallet.address, 0)).balance).equal(bigNumberify(0));

        // Commit block with eth partial exit.
        const exitValue = parseEther("0.2");
        const exitBlockPublicData = createWithdrawPublicData(0, exitValue, exitWallet.address);
        const commitExitTx = await franklinDeployedContract.commitBlock(2, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            exitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        await commitExitTx.wait();

        // Verify block with deposit and exit.
        await franklinDeployedContract.verifyBlock(1, dummyBlockProof, {gasLimit: bigNumberify("100000")});
        const verify2 = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, {gasLimit: bigNumberify("100000")});
        await verify2.wait();

        expect((await franklinDeployedContract.balances(exitWallet.address, 0)).balance).equal(exitValue);

        // Withdraw eth.
        const oldBalance = await exitWallet.getBalance();
        const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);
        const exitTx = await exitWalletFranklinContract.withdrawETH(exitValue);
        const exitTxReceipt = await exitTx.wait();
        const gasUsed = exitTxReceipt.gasUsed.mul(await provider.getGasPrice());
        const newBalance = await exitWallet.getBalance();
        expect(newBalance.sub(oldBalance).add(gasUsed)).eq(exitValue);
        expect((await franklinDeployedContract.balances(exitWallet.address, 0)).balance).equal(bigNumberify(0));
    });

    it("ERC20 deposit with commit", async () => {
        const depositValue = bigNumberify("78");
        const depositFee = bigNumberify("8");
        await erc20DeployedToken.approve(franklinDeployedContract.address, depositValue);

        const tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, depositValue, franklinAddressBinary,
            {gasLimit: bigNumberify("150000")});
        const receipt = await tx.wait();
        const event = receipt.events.pop().args;

        expect(event.owner).equal(wallet.address);
        expect(event.tokenId).equal(1);
        expect(event.amount).equal(depositValue);
        expect(event.franklinAddress).equal("0x" + franklinAddress);

        expect((await franklinDeployedContract.balances(wallet.address, 1)).balance).equal(depositValue);
        expect(await franklinDeployedContract.depositWasDone(wallet.address)).equal(true);
        expect(await franklinDeployedContract.depositFranklinToETH(franklinAddressBinary)).equal(wallet.address);

        const depositBlockPublicData = createDepositPublicData(1, depositValue.sub(depositFee), depositFee, franklinAddress);
        const commitTx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        await commitTx.wait();
        expect((await franklinDeployedContract.balances(wallet.address, 1)).balance).equal(bigNumberify(0));

        // Commit block with erc20 partial exit.
        const exitValue = bigNumberify("45");
        const exitBlockPublicData = createWithdrawPublicData(1, exitValue, exitWallet.address);
        const commitExitTx = await franklinDeployedContract.commitBlock(2, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            exitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        await commitExitTx.wait();

        // Verify block with deposit and exit.
        await franklinDeployedContract.verifyBlock(1, dummyBlockProof, {gasLimit: bigNumberify("100000")});
        const verify2 = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, {gasLimit: bigNumberify("100000")});
        await verify2.wait();

        expect((await franklinDeployedContract.balances(exitWallet.address, 1)).balance).equal(exitValue);

        // Withdraw erc20.
        const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);
        const exitTx = await exitWalletFranklinContract.withdrawERC20(erc20DeployedToken.address, exitValue);
        const recp = await exitTx.wait();
        expect(await erc20DeployedToken.balanceOf(exitWallet.address)).eq(exitValue);
        expect((await franklinDeployedContract.balances(exitWallet.address, 1)).balance).equal(bigNumberify(0));
    });
});

function createDepositPublicData(tokenId, amount: BigNumber, fee: BigNumber, franklinAddress: string): Buffer {
    const txId = Buffer.from("01", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(0, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    const amountBytes = packAmount(new BN(amount.toString()));
    const feeBytes = packFee(new BN(fee.toString()));
    const addressBytes = Buffer.from(franklinAddress, "hex");
    const padBytes = Buffer.alloc(3, 0);

    return Buffer.concat([txId, accountId, tokenBytes, amountBytes, feeBytes, addressBytes, padBytes]);
}

function createWithdrawPublicData(tokenId, amount: BigNumber, ethAddress: string): Buffer {
    const txId = Buffer.from("03", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(0, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    const amountBytes = packAmount(new BN(amount.toString()));
    const feeBytes = packFee(new BN("0"));
    const addressBytes = Buffer.from(ethAddress.substr(2), "hex");
    const padBytes = Buffer.alloc(2, 0);

    return Buffer.concat([txId, accountId, tokenBytes, amountBytes, feeBytes, addressBytes, padBytes]);
}
