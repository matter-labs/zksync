import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, deployGovernance} from "./deploy";

import {expect, use, assert} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther, hexlify, BigNumber} from "ethers/utils";
import {createDepositPublicData, createWithdrawPublicData, createFullExitPublicData, hex_to_ascii} from "./helpers"

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "0809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("FAILS", function() {
    this.timeout(50000);

    let franklinDeployedContract;
    let governanceDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        governanceDeployedContract = await deployGovernance(wallet, wallet.address);
        franklinDeployedContract = await deployFranklin(wallet, governanceDeployedContract.address);
        erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
        // Make sure that exit wallet can execute transactions.
        // await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("Deposit", async () => {
        // ETH: Wrong tx value (msg.value >= fee)
        const depositETH1Value = parseEther("0.005"); // the value passed to tx
        let tx1 = await franklinDeployedContract.depositETH(
            franklinAddressBinary,
            {
                value: depositETH1Value,
                gasLimit: bigNumberify("500000")
            }
        );

        await tx1.wait()
        .catch(() => {});

        const code1 = await provider.call(tx1, tx1.blockNumber);
        const reason1 = hex_to_ascii(code1.substr(138));
        
        expect(reason1.substring(0,5)).equal("fdh11");

        // ETH: Wrong tx value (amount <= MAX_VALUE)
        const depositETH2Value = parseEther("340282366920938463463.374607431768211456"); // the value passed to tx
        let tx2 = await franklinDeployedContract.depositETH(
            franklinAddressBinary,
            {
                value: depositETH2Value,
                gasLimit: bigNumberify("500000")
            }
        );

        await tx2.wait()
        .catch(() => {});

        const code2 = await provider.call(tx2, tx2.blockNumber);
        const reason2 = hex_to_ascii(code2.substr(138));
        
        expect(reason2.substring(0,5)).equal("fdh12");

        // ERC20: Wrong tx value (msg.value >= fee)
        let erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);

        const depositERCValue = 78;
        const feeValue = parseEther("0.001");
        await erc20DeployedToken.approve(franklinDeployedContract.address, depositERCValue);

        let tx3 = await franklinDeployedContract.depositERC20(
            erc20DeployedToken.address,
            depositERCValue, 
            franklinAddressBinary,
            {value: feeValue, gasLimit: bigNumberify("500000")}
        );

        await tx3.wait()
        .catch(() => {});

        const code3 = await provider.call(tx3, tx3.blockNumber);
        const reason3 = hex_to_ascii(code3.substr(138));
        
        expect(reason3.substring(0,5)).equal("fd011");
    });

    it("Exodus Mode", async () => {
        // Deposit eth
        const depositValue = parseEther("0.3"); // the value passed to tx
        const depositAmount = parseEther("0.293775816"); // amount after: tx value - some counted fee
        let tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: depositValue});
        let receipt = await tx.wait();
        let events = receipt.events;

        const priorityEvent = events[0].args;
        const depositEvent = events[1].args;

        expect(priorityEvent.opType).equal(1);
        expect(priorityEvent.pubData).equal("0x52312ad6f01657413b2eae9287f6b9adad93d5fe000000000000000000000413b38c5447d0000809101112131415161718192021222334252627");
        expect(priorityEvent.fee).equal(bigNumberify("0x161cdcc4563000"));

        expect(depositEvent.owner).equal(wallet.address);
        expect(depositEvent.tokenId).equal(0);
        expect(depositEvent.amount).equal(depositAmount);
        expect(depositEvent.franklinAddress).equal("0x" + franklinAddress);

        let totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        let firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(1);
        expect(firstPriorityRequestId).equal(0);

        // Commit block with eth deposit
        const depositBlockPublicData = createDepositPublicData(0, hexlify(depositAmount), franklinAddress);
        tx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent1 = events[0].args;
        
        expect(commitedEvent1.blockNumber).equal(1);
        
        let totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(1);
        
        expect((await franklinDeployedContract.blocks(1)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).commitment).equal("0xa26d1ddaa435d774cd54b089570fb6d9e94938b46755453f6bfb2f74d7c31776");
        expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(1)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

    });
});
