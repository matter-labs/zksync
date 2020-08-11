import {Contract, ethers, constants, BigNumber} from "ethers";
import { parseEther} from "ethers/lib/utils";
import {ETHProxy} from "zksync";
import {Address, TokenAddress} from "zksync/build/types";
import {
    Deployer, readContractCode, readTestContracts,
} from "../../src.ts/deploy";

const {simpleEncode} = require("ethereumjs-abi");
const {expect} = require("chai");
const {deployContract} = require("ethereum-waffle");
const {wallet, exitWallet, deployTestContract, getCallRevertReason, IERC20_INTERFACE} = require("./common");
import * as zksync from "zksync";

async function onchainTokenBalanceOfContract(ethWallet: ethers.Wallet, contractAddress: Address, token: Address): Promise<BigNumber> {
    const erc20contract = new Contract(
        token,
        IERC20_INTERFACE.abi,
        ethWallet,
    );
    return BigNumber.from(await erc20contract.balanceOf(contractAddress));
}

async function onchainBalance(ethWallet: ethers.Wallet, token: Address): Promise<BigNumber> {
    if (token === ethers.constants.AddressZero) {
        return ethWallet.getBalance();
    } else {
        const erc20contract = new Contract(
            token,
            IERC20_INTERFACE.abi,
            ethWallet,
        );
        return BigNumber.from(await erc20contract.balanceOf(ethWallet.address));
    }
}

describe("zkSync process tokens which have no return value in `transfer` and `transferFrom` calls", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZkSyncWithdrawalUnitTest");
        const deployer = new Deployer({deployWallet: wallet, contracts});
        await deployer.deployAll({gasLimit: 6500000});
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("MintableERC20NoTransferReturnValueTest"), [],
            {gasLimit: 5000000},
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });
    });

    async function performWithdraw(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        const contractBalanceBefore = BigNumber.from((await zksyncContract.getBalanceToWithdraw(ethWallet.address, tokenId)));
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawETH(amount);
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawERC20(token, amount);
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance = token == constants.AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), "withdraw account balance mismatch").eq(expectedBalance.toString());

        const contractBalanceAfter = BigNumber.from((await zksyncContract.getBalanceToWithdraw(ethWallet.address, tokenId)));
        const expectedContractBalance = contractBalanceBefore.sub(amount);
        expect(contractBalanceAfter.toString(), "withdraw contract balance mismatch").eq(expectedContractBalance.toString());
    }

    it("Deposit ERC20 success", async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther("1.0");
        await tokenContract.approve(zksyncContract.address, depositAmount);

        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);
        await expect(zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address))
            .to.emit(zksyncContract, "OnchainDeposit")
            .withArgs(wallet.address, tokenId, depositAmount, wallet.address);
    });

    it("Deposit ERC20 fail", async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther("1.0");
        await tokenContract.approve(zksyncContract.address, depositAmount.div(2));

        const balanceBefore = await tokenContract.balanceOf(wallet.address);
        const {revertReason} = await getCallRevertReason(async () => await zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address));
        const balanceAfter = await tokenContract.balanceOf(wallet.address);
        expect(balanceBefore).eq(balanceAfter);
    });

    it("Withdraw ERC20 success", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount.mul(2));
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        const onchainBalBefore_first_subtest = await onchainBalance(wallet, tokenContract.address);
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount);
        const onchainBalAfter_first_subtest = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter_first_subtest.sub(onchainBalBefore_first_subtest)).eq(withdrawAmount.toString());

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        const onchainBalBefore_second_subtest = await onchainBalance(wallet, tokenContract.address);
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.div(2));
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.div(2));
        const onchainBalAfter_second_subtest = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter_second_subtest.sub(onchainBalBefore_second_subtest)).eq(withdrawAmount.toString());
    });

    it("Withdraw ERC20 incorrect amount", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount);
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);

        const onchainBalBefore = await onchainBalance(wallet, tokenContract.address);
        const {revertReason} = await getCallRevertReason(async () => await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.add(1)));
        const onchainBalAfter = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter).eq(onchainBalBefore);
    });

    it("Complete pending withdawals", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");
        const withdrawsToCancel = 5;

        await tokenContract.transfer(zksyncContract.address, withdrawAmount);

        for (const tokenAddress of [tokenContract.address]) {
            const tokenId = await ethProxy.resolveTokenId(tokenAddress);

            await zksyncContract.setBalanceToWithdraw(exitWallet.address, tokenId, 0);
            await zksyncContract.addPendingWithdrawal(exitWallet.address, tokenId, withdrawAmount.div(2));
            await zksyncContract.addPendingWithdrawal(exitWallet.address, tokenId, withdrawAmount.div(2));

            const onchainBalBefore = await onchainBalance(exitWallet, tokenAddress);

            await zksyncContract.completeWithdrawals(withdrawsToCancel);

            const onchainBalAfter = await onchainBalance(exitWallet, tokenAddress);

            expect(onchainBalAfter.sub(onchainBalBefore)).eq(withdrawAmount.toString());
        }
    });
});

describe("zkSync process tokens which take fee from sender", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let FEE_AMOUNT;
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZkSyncWithdrawalUnitTest");
        const deployer = new Deployer({deployWallet: wallet, contracts});
        await deployer.deployAll({gasLimit: 6500000});
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("MintableERC20FeeAndDividendsTest"), [true, true],
            {gasLimit: 5000000},
        );
        FEE_AMOUNT = BigNumber.from((await tokenContract.FEE_AMOUNT_AS_VALUE()));
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });
    });

    async function performWithdrawNoCheckContractBalance(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawETH(amount);
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawERC20(token, amount);
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance = token == constants.AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), "withdraw account balance mismatch").eq(expectedBalance.toString());
    }

    it("Withdraw ERC20 success", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount.mul(2));
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);


        // test one withdrawal
        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        const onchainBalBefore_first_subtest = await onchainBalance(wallet, tokenContract.address);

        await performWithdrawNoCheckContractBalance(wallet, tokenContract.address, tokenId, withdrawAmount.sub(FEE_AMOUNT));
        expect(await zksyncContract.getBalanceToWithdraw(wallet.address, tokenId)).eq("0");
        expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq(withdrawAmount);

        const onchainBalAfter_first_subtest = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter_first_subtest.sub(onchainBalBefore_first_subtest)).eq(withdrawAmount.sub(FEE_AMOUNT).toString());


        // test two withdrawals
        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        const onchainBalBefore_second_subtest = await onchainBalance(wallet, tokenContract.address);

        await performWithdrawNoCheckContractBalance(wallet, tokenContract.address, tokenId, withdrawAmount.div(2).sub(FEE_AMOUNT));
        expect(await zksyncContract.getBalanceToWithdraw(wallet.address, tokenId)).eq(withdrawAmount.div(2).toString());
        expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq(withdrawAmount.div(2).toString());
        await performWithdrawNoCheckContractBalance(wallet, tokenContract.address, tokenId, withdrawAmount.div(2).sub(FEE_AMOUNT));
        expect(await zksyncContract.getBalanceToWithdraw(wallet.address, tokenId)).eq("0");
        expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq("0");

        const onchainBalAfter_second_subtest = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter_second_subtest.sub(onchainBalBefore_second_subtest)).eq(withdrawAmount.sub(2 * FEE_AMOUNT).toString());
    });

    it("Complete pending withdawals => should not complete transfer because of token fee", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");
        const withdrawsToCancel = 5;

        await tokenContract.transfer(zksyncContract.address, withdrawAmount);

        for (const tokenAddress of [tokenContract.address]) {
            const tokenId = await ethProxy.resolveTokenId(tokenAddress);

            await zksyncContract.setBalanceToWithdraw(exitWallet.address, tokenId, 0);
            await zksyncContract.addPendingWithdrawal(exitWallet.address, tokenId, withdrawAmount);

            const onchainBalBefore = await onchainBalance(exitWallet, tokenAddress);

            await zksyncContract.completeWithdrawals(withdrawsToCancel);

            const onchainBalAfter = await onchainBalance(exitWallet, tokenAddress);

            expect(onchainBalAfter).eq(onchainBalBefore);

            expect(await zksyncContract.getBalanceToWithdraw(exitWallet.address, tokenId)).eq(withdrawAmount);

            // contract balance should not change
            expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq(withdrawAmount);
        }
    });
});

describe("zkSync process tokens which take fee from recipient", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let FEE_AMOUNT;
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZkSyncWithdrawalUnitTest");
        const deployer = new Deployer({deployWallet: wallet, contracts});
        await deployer.deployAll({gasLimit: 6500000});
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("MintableERC20FeeAndDividendsTest"), [true, false],
            {gasLimit: 5000000},
        );
        FEE_AMOUNT = BigNumber.from((await tokenContract.FEE_AMOUNT_AS_VALUE()));
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });
    });

    it("Make a deposit of tokens that should take a fee from recipient contract", async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther("1.0");
        await tokenContract.approve(zksyncContract.address, depositAmount);

        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);
        await expect(zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address))
            .to.emit(zksyncContract, "OnchainDeposit")
            .withArgs(wallet.address, tokenId, depositAmount.sub(FEE_AMOUNT), wallet.address);
    });
});

describe("zkSync process tokens which adds dividends to recipient", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let DIVIDEND_AMOUNT;
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZkSyncWithdrawalUnitTest");
        const deployer = new Deployer({deployWallet: wallet, contracts});
        await deployer.deployAll({gasLimit: 6500000});
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("MintableERC20FeeAndDividendsTest"), [false, false],
            {gasLimit: 5000000},
        );
        DIVIDEND_AMOUNT = BigNumber.from((await tokenContract.DIVIDEND_AMOUNT_AS_VALUE()));
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });
    });

    it("Make a deposit of tokens that should adds dividends to the recipient", async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther("1.0");
        await tokenContract.approve(zksyncContract.address, depositAmount);

        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);
        await expect(zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address))
            .to.emit(zksyncContract, "OnchainDeposit")
            .withArgs(wallet.address, tokenId, depositAmount.add(DIVIDEND_AMOUNT), wallet.address);
    });
});
