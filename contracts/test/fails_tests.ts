import {ethers} from "ethers";
import {
    addTestERC20Token,
    addTestNotApprovedERC20Token,
    mintTestERC20Token,
    Deployer,
} from "../src.ts/deploy";
import {expect, use} from "chai";
const { createMockProvider, getWallets, solidity } = require("ethereum-waffle");
import {bigNumberify, hexlify, parseEther} from "ethers/utils";
import {
    cancelOustandingDepositsForExodus, CHUNKS_SIZE,
    createDepositPublicData,
    createNoopPublicData,
    createWrongDepositPublicData,
    createWrongNoopPublicData,
    createWrongOperationPublicData,
    hex_to_ascii, OPERATIONS,
    postBlockCommit,
    postBlockVerify,
    postErc20Deposit,
    postEthDeposit,
    postFullExit,
    withdrawErcFromContract,
    withdrawEthFromContract,
} from "./helpers";

use(solidity);

// For: geth

// const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
// const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
// const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);

// For: ganache

const provider = createMockProvider() //{gasLimit: 7000000, gasPrice: 2000000000});
const [wallet, exitWallet]  = getWallets(provider);

const franklinAddress = "0809101112131415161718192021222334252627";
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];
const PRIORITY_QUEUE_EXIRATION = 16;

describe("PLANNED FAILS", function () {
    this.timeout(100000);
    provider.pollingInterval = 100; // faster deploys/txs on localhost

    const deployer = new Deployer(wallet, true);
    let franklinDeployedContract;
    let governanceDeployedContract;
    let verifierDeployedContract;
    let priorityQueueDeployedContract;
    let erc20DeployedToken1;
    let erc20DeployedToken2;

    beforeEach(async () => {
        console.log("---\n");
        verifierDeployedContract = await deployer.deployVerifier();
        governanceDeployedContract = await deployer.deployGovernance();
        priorityQueueDeployedContract = await deployer.deployPriorityQueue();
        franklinDeployedContract = await deployer.deployFranklin();
        await governanceDeployedContract.setValidator(wallet.address, true);
        erc20DeployedToken1 = await addTestERC20Token(wallet, governanceDeployedContract);
        erc20DeployedToken2 = await addTestNotApprovedERC20Token(wallet);
        await mintTestERC20Token(wallet, erc20DeployedToken1);
        await mintTestERC20Token(wallet, erc20DeployedToken2);
        // Make sure that exit wallet can execute transactions.
        await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("Onchain errors", async () => {
        // ETH deposit: Wrong tx value (msg.value is too low)
        console.log("\n - ETH deposit: Wrong tx value (msg.value is too low) started");
        const depositETH1Value = parseEther("0.003"); // the value passed to tx must be too low
        const depositAmount = parseEther("0.0000000000000001"); // amount after: tx value - some counted fee
        await postEthDeposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositAmount,
            null,
            franklinAddress,
            depositETH1Value,
            "fdh11",
        );
        console.log(" + ETH deposit: Wrong tx value (msg.value is too low) passed");

        // ERC20 deposit: Wrong tx value (msg.value < fee)
        console.log("\n - ERC20 deposit: Wrong tx value (msg.value < fee) started");
        const depositERCValue = 78;
        const notCorrectFeeValue = parseEther("0.001");

        await postErc20Deposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            erc20DeployedToken1,
            depositERCValue,
            null,
            franklinAddress,
            notCorrectFeeValue,
            "fd011",
        );

        console.log(" + ERC20 deposit: Wrong tx value (msg.value < fee) passed");

        // ERC20 deposit: Wrong token address
        console.log("\n - ERC20 deposit: Wrong token address started");

        const correctFeeValue = parseEther("0.3");
        await postErc20Deposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            erc20DeployedToken2,
            depositERCValue,
            null,
            franklinAddress,
            correctFeeValue,
            "gvs11",
        );

        console.log(" + ERC20 deposit: Wrong token address passed");

        // ETH withdraw: balance error
        console.log("\n - ETH withdraw: balance error started");
        const balanceToWithdraw1 = "0x01A2FED090BCD000";
        await withdrawEthFromContract(
            provider,
            wallet,
            franklinDeployedContract,
            balanceToWithdraw1,
            "frw11",
        );
        console.log(" + ETH withdraw: balance error passed");

        // ERC20 withdraw: Wrong token address
        console.log("\n - ERC20 withdraw: Wrong token address started");
        await withdrawErcFromContract(
            provider,
            wallet,
            franklinDeployedContract,
            erc20DeployedToken2,
            1,
            balanceToWithdraw1,
            "gvs11",
        );
        console.log(" + ERC20 withdraw: Wrong token address passed");

        // Full Exit: Wrong token address
        console.log("\n - Full Exit: Wrong token address started");
        const value = parseEther("0.3"); // the value passed to tx
        const accountId = 0;
        await postFullExit(
            provider,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            accountId,
            erc20DeployedToken2.address,
            value,
            "gvs11",
        );
        console.log(" + Full Exit: Wrong token address passed");

        // Full Exit: Wrong tx value (lower than fee)
        console.log("\n - Full Exit: Wrong tx value (lower than fee) started");
        const wrongValue = parseEther("0.001"); // the value passed to tx
        await postFullExit(
            provider,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            accountId,
            erc20DeployedToken1.address,
            wrongValue,
            "fft11",
        );
        console.log(" + Full Exit: Wrong tx value (lower than fee) passed");

    });

    it("Enter Exodus Mode external caller", async () => {
        const depositValue = parseEther("10");
        const depositAmount = parseEther("9.996778");
        const depositFee = parseEther("0.003222");
        await postEthDeposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositAmount,
            depositFee,
            franklinAddress,
            depositValue,
            null,
        );
        const blockNumberSinceLastDeposit = await provider.getBlockNumber();

        let tx = await (await franklinDeployedContract.triggerExodusIfNeeded()).wait();
        let isExodusTriggered = await franklinDeployedContract.exodusMode();
        expect(tx.status, "Asking for exodus should succeed").eq(1);
        expect(isExodusTriggered, "Exodus should not be triggered").eq(false);

        while (await provider.getBlockNumber() - blockNumberSinceLastDeposit < PRIORITY_QUEUE_EXIRATION) {
            await new Promise((r) => setTimeout(r, 300));
        }

        tx = await (await franklinDeployedContract.triggerExodusIfNeeded()).wait();
        isExodusTriggered = await franklinDeployedContract.exodusMode();
        expect(tx.status, "Asking for exodus should succeed").eq(1);
        expect(isExodusTriggered, "Exodus should be triggered after priority expiration").eq(true);
    });

    it("Enter Exodus Mode with commit", async () => {
        console.log("\n - test Exodus Mode started");

        let depositsToCancel;

        const depositValue = parseEther("10");
        const depositAmount = parseEther("9.996778"); // amount after: tx value - some counted fee
        const depositFee = parseEther("0.003222"); // tx fee

        let blockNumberSinceLastDeposit = await provider.getBlockNumber();
        for (let i = 0; i < 5; i++) {
            await postEthDeposit(
                provider,
                wallet,
                franklinDeployedContract,
                priorityQueueDeployedContract,
                depositAmount,
                depositFee,
                franklinAddress,
                depositValue,
                null,
            );
            blockNumberSinceLastDeposit = await provider.getBlockNumber();
            console.log(`Posted ${i + 1} deposit`);
        }

        // Try to cancel deposits in not exodus mode
        depositsToCancel = 5;
        await cancelOustandingDepositsForExodus(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositsToCancel,
            null,
            null,
            "frc11",
        );

        console.log("Cancel deposits before exodus is triggered failed, ok");


        while (await provider.getBlockNumber() - blockNumberSinceLastDeposit < PRIORITY_QUEUE_EXIRATION) {
            await new Promise((r) => setTimeout(r, 300));
        }

        // Get commit exodus mode revert code
        const noopBlockPublicData = createNoopPublicData();
        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            noopBlockPublicData,
            null,
            null,
            null,
            null,
            true,
        );

        // Get commit exodus event
        const exodus = await franklinDeployedContract.exodusMode();
        expect(exodus, "exodus mode is not triggered").equal(true);

        console.log("Exodus mode triggered");

        // Get deposit exodus mode revert code
        await postEthDeposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositAmount,
            depositFee,
            franklinAddress,
            depositValue,
            "fre11",
        );

        console.log("Got exodus mode deposit tx revert code");

        // Cancel first 2 deposits
        depositsToCancel = 2;
        await cancelOustandingDepositsForExodus(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositsToCancel,
            depositsToCancel,
            parseEther("19.993556"),
            null,
        );

        console.log(`Canceled ${depositsToCancel} deposits`);

        // Cancel 1 deposit
        depositsToCancel = 1;
        await cancelOustandingDepositsForExodus(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositsToCancel,
            depositsToCancel,
            parseEther("29.990334"),
            null,
        );

        console.log(`Canceled ${depositsToCancel} deposits`);

        // Cancel last deposits - try 5 but there is only 2 left
        const depositsLeft = await priorityQueueDeployedContract.totalOpenPriorityRequests();
        depositsToCancel = 5;
        await cancelOustandingDepositsForExodus(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositsToCancel,
            depositsLeft,
            parseEther("49.983890"),
            null,
        );

        console.log(`Tried to cancel ${depositsToCancel}, canceled last ${depositsLeft} deposits`);

        // Try to cancel more deposits must fail
        await cancelOustandingDepositsForExodus(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositsToCancel,
            null,
            null,
            "pgs11",
        );

        console.log("Got revert code when there are no requests to cancel");

        // Withdraw eth
        const rollupBalance = await franklinDeployedContract.balancesToWithdraw(wallet.address, 0);
        await withdrawEthFromContract(
            provider,
            wallet,
            franklinDeployedContract,
            rollupBalance,
            null,
        );

        console.log("Balances withdrawed");

        console.log(" + test Exodus Mode passed");
    });

    it("Block commit errors", async () => {
        const noopBlockPublicData = createNoopPublicData();

        // Wrong commit number
        console.log("\n - Wrong commit number started");

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            2,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            noopBlockPublicData,
            null,
            null,
            null,
            "fck11",
        );
        console.log(" + Wrong commit number passed");

        // Wrong noop pubdata - less length
        console.log("\n - Wrong noop pubdata - less length started");
        const wrongNoopBlockPublicData = createWrongNoopPublicData();

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            wrongNoopBlockPublicData,
            null,
            null,
            null,
            "fcs11",
        );
        console.log(" + Wrong noop pubdata - less length passed");

        // Wrong deposit pubdata - less length
        console.log("\n - Wrong deposit pubdata - less length started");
        let depositAmount = parseEther("0.3");
        const wrongDepositBlockPublicData = createWrongDepositPublicData(0, hexlify(depositAmount), franklinAddress);

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            wrongDepositBlockPublicData,
            null,
            null,
            null,
            "bse11",
        );
        console.log(" + Wrong deposit pubdata - less length passed");

        // Wrong operation id
        console.log("\n - Wrong operation pubdata - wrong op id started");
        const wrongOperationPublicData = createWrongOperationPublicData();

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            wrongOperationPublicData,
            null,
            null,
            null,
            "fpp14",
        );
        console.log(" + Wrong operation pubdata - wrong op id passed");

        // Wrong priority operation - non existed
        console.log("\n - Wrong priority operation - non existed started");
        const depositPublicData = createDepositPublicData(0, hexlify(depositAmount), franklinAddress);

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            depositPublicData,
            null,
            null,
            null,
            "pvs11",
        );
        console.log(" + Wrong priority operation - non existed passed");

        // Wrong priority operation - different data
        console.log("\n - Wrong priority operation - different data started");
        const depositValue = parseEther("0.3");
        const depositCorrectAmount = parseEther("0.296778");
        const depositFee = parseEther("0.003222");
        await postEthDeposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositCorrectAmount,
            depositFee,
            franklinAddress,
            depositValue,
            null,
        );

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            depositPublicData, // the part that went to fee will not be taken into account
            null,
            null,
            null,
            "fvs11",
        );
        console.log(" + Wrong priority operation - different data passed");

        // Not governor commit
        console.log("\n - Not governor started");
        const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);

        await postBlockCommit(
            provider,
            wallet,
            exitWalletFranklinContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            noopBlockPublicData,
            null,
            null,
            null,
            "grr21",
        );
        console.log(" + Not governor passed");
    });

    it("Block verify errors", async () => {
        const noopBlockPublicData = createNoopPublicData();

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            22,
            "0000000000000000000000000000000000000000000000000000000000000000",
            noopBlockPublicData,
            0,
            0,
            "0x33d02993f84da5cb2bb46cce92ded88d8484e46eee4ee3fe9e3db6cfbbd9f9a7",
            null,
        );

        console.log("Block committed");

        // Wrong commit number
        console.log("\n - Wrong verify number started");

        const wrongBlockNumber = 2;

        await postBlockVerify(
            provider,
            franklinDeployedContract,
            wrongBlockNumber,
            dummyBlockProof,
            "fvk11",
        );

        console.log(" + Wrong verify number passed");

        // Not governor commit
        console.log("\n - Not governor started");

        const blockNumber = 1;
        const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);
        await postBlockVerify(
            provider,
            exitWalletFranklinContract,
            blockNumber,
            dummyBlockProof,
            "grr21",
        );
        console.log(" + Not governor passed");
    });

    it("Enter blocks revert", async () => {
        console.log("\n - Blocks revert started");
        const noopBlockPublicData = createNoopPublicData();

        let reverted = false;
        let i = 0;
        let blockNumberSinceLastBlock = await provider.getBlockNumber();
        for (i = 0; i < 2; i++) {
            expect(await franklinDeployedContract.totalBlocksCommitted()).equal(i);
            const tx = await franklinDeployedContract.commitBlock(i + 1, 22,
                Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
                noopBlockPublicData,
                {
                    gasLimit: bigNumberify("500000"),
                },
            );
            await tx.wait();
            blockNumberSinceLastBlock = await provider.getBlockNumber();
        }

        while (await provider.getBlockNumber() - blockNumberSinceLastBlock < 8) {
        }

        const tx = await franklinDeployedContract.commitBlock(i + 1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            noopBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        const receipt = await tx.wait();

        const event = receipt.events.pop();
        if (event.event == "BlocksReverted") {
            expect(await franklinDeployedContract.totalBlocksCommitted()).equal(0);
            reverted = true;
        }

        expect(reverted, "reverted event expected").equal(true);
        console.log(" + Blocks revert passed");
    });

    it("Priority Queue errors", async () => {
        console.log("\n - Set franklin address twice will not work started");
        // Set franklin address again

        const prTx2 = await priorityQueueDeployedContract.setFranklinAddress(
            franklinDeployedContract.address,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        await prTx2.wait()
            .catch(() => {
            });

        const code1 = await provider.call(prTx2, prTx2.blockNumber);
        const reason1 = hex_to_ascii(code1.substr(138));

        expect(reason1.substring(0, 5)).equal("pcs11");
        console.log(" + Set franklin address twice will not work passed");
    });

    it("Commit block small priority op chunk", async () => {
        const shortendPubdataPriorityOps = [];

        const shortDepositPubdata = Buffer.alloc((OPERATIONS.deposit.chunks - 1) * CHUNKS_SIZE, 0);
        shortDepositPubdata[0] = OPERATIONS.deposit.id; // set correct op type
        shortendPubdataPriorityOps.push(shortDepositPubdata);

        const shortFullExitPubdata = Buffer.alloc((OPERATIONS.fullExit.chunks - 1) * CHUNKS_SIZE, 0);
        shortFullExitPubdata[0] = OPERATIONS.fullExit.id; // set correct op type
        shortendPubdataPriorityOps.push(shortFullExitPubdata);

        const shortChangePubkeyOnchainPubdata = Buffer.alloc((OPERATIONS.changePubkeyOnchain.chunks - 1) * CHUNKS_SIZE, 0);
        shortChangePubkeyOnchainPubdata[0] = OPERATIONS.changePubkeyOnchain.id; // set correct op type
        shortendPubdataPriorityOps.push(shortChangePubkeyOnchainPubdata);

        const shortWithdrawPubdata = Buffer.alloc((OPERATIONS.withdraw.chunks - 1) * CHUNKS_SIZE, 0);
        shortWithdrawPubdata[0] = OPERATIONS.withdraw.id; // set correct op type
        shortendPubdataPriorityOps.push(shortWithdrawPubdata);

        for (const shortPubdata of shortendPubdataPriorityOps) {
            await postBlockCommit(
                provider,
                wallet,
                franklinDeployedContract,
                1,
                22,
                "0000000000000000000000000000000000000000000000000000000000000000",
                shortPubdata,
                null,
                null,
                null,
                "bse11",
            );
        }
    });
});
