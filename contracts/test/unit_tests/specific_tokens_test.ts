import { Contract, ethers, constants, BigNumber } from 'ethers';
import { parseEther } from 'ethers/lib/utils';
import { ETHProxy } from 'zksync';
import { Address, TokenAddress } from 'zksync/build/types';
import { Deployer, readContractCode, readProductionContracts } from '../../src.ts/deploy';
import { ZkSyncWithdrawalUnitTestFactory } from '../../typechain';

const hardhat = require('hardhat');
const { expect } = require('chai');
const { getCallRevertReason, IERC20_INTERFACE, DEFAULT_REVERT_REASON } = require('./common');

let wallet, exitWallet;

async function onchainTokenBalanceOfContract(
    ethWallet: ethers.Wallet,
    contractAddress: Address,
    token: Address
): Promise<BigNumber> {
    const erc20contract = new Contract(token, IERC20_INTERFACE.abi, ethWallet);
    return BigNumber.from(await erc20contract.balanceOf(contractAddress));
}

async function onchainBalance(ethWallet: ethers.Wallet, token: Address): Promise<BigNumber> {
    if (token === ethers.constants.AddressZero) {
        return ethWallet.getBalance();
    } else {
        const erc20contract = new Contract(token, IERC20_INTERFACE.abi, ethWallet);
        return BigNumber.from(await erc20contract.balanceOf(ethWallet.address));
    }
}

describe('zkSync process tokens which have no return value in `transfer` and `transferFrom` calls', function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    before(async () => {
        [wallet, exitWallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncWithdrawalUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncWithdrawalUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractDeployFactory = await hardhat.ethers.getContractFactory(
            'MintableERC20NoTransferReturnValueTest'
        );
        tokenContract = await tokenContractDeployFactory.deploy();
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address
        });
    });

    async function performWithdraw(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        const contractBalanceBefore = BigNumber.from(await zksyncContract.getPendingBalance(ethWallet.address, token));
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawPendingBalance(
                ethWallet.address,
                ethers.constants.AddressZero,
                amount
            );
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawPendingBalance(ethWallet.address, token, amount);
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance =
            token == constants.AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), 'withdraw account balance mismatch').eq(expectedBalance.toString());

        const contractBalanceAfter = BigNumber.from(await zksyncContract.getPendingBalance(ethWallet.address, token));
        const expectedContractBalance = contractBalanceBefore.sub(amount);
        expect(contractBalanceAfter.toString(), 'withdraw contract balance mismatch').eq(
            expectedContractBalance.toString()
        );
    }

    it('Deposit ERC20 success', async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther('1.0');
        await tokenContract.approve(zksyncContract.address, depositAmount);

        await zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address);
    });

    it('Deposit ERC20 fail', async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther('1.0');
        await tokenContract.approve(zksyncContract.address, depositAmount.div(2));

        const balanceBefore = await tokenContract.balanceOf(wallet.address);
        try {
            const { revertReason } = await getCallRevertReason(
                async () => await zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address)
            );
            expect(revertReason).to.not.equal(DEFAULT_REVERT_REASON);
        } catch (e) {}
        const balanceAfter = await tokenContract.balanceOf(wallet.address);

        expect(balanceBefore).eq(balanceAfter);
    });

    it('payoutAmount success', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

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

    it('Withdraw ERC20 incorrect amount', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount);
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);

        const onchainBalBefore = await onchainBalance(wallet, tokenContract.address);
        const { revertReason } = await getCallRevertReason(
            async () => await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.add(1))
        );
        const onchainBalAfter = await onchainBalance(wallet, tokenContract.address);

        expect(onchainBalAfter).eq(onchainBalBefore);
        expect(revertReason).to.not.eq(DEFAULT_REVERT_REASON);
    });

    it('Complete pending withdawals', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        await tokenContract.transfer(zksyncContract.address, withdrawAmount);

        for (const tokenAddress of [tokenContract.address]) {
            const tokenId = await ethProxy.resolveTokenId(tokenAddress);

            await zksyncContract.setBalanceToWithdraw(exitWallet.address, tokenId, withdrawAmount);

            const onchainBalBefore = await onchainBalance(exitWallet, tokenAddress);

            await zksyncContract.withdrawOrStoreExternal(tokenId, exitWallet.address, withdrawAmount);

            const onchainBalAfter = await onchainBalance(exitWallet, tokenAddress);

            expect(onchainBalAfter.sub(onchainBalBefore)).eq(withdrawAmount.toString());
        }
    });
});

describe('zkSync process tokens which take fee from sender', function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let FEE_AMOUNT;
    before(async () => {
        [wallet, exitWallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncWithdrawalUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncWithdrawalUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractDeployFactory = await hardhat.ethers.getContractFactory('MintableERC20FeeAndPayoutTest');
        tokenContract = await tokenContractDeployFactory.deploy(true, true);
        FEE_AMOUNT = BigNumber.from(await tokenContract.feeAmount());
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address
        });
    });

    async function performWithdrawNoCheckContractBalance(
        ethWallet: ethers.Wallet,
        token: TokenAddress,
        tokenId: number,
        amount: BigNumber
    ) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawPendingBalance(
                ethWallet.address,
                ethers.constants.AddressZero,
                amount
            );
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawPendingBalance(ethWallet.address, token, amount);
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance =
            token == constants.AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), 'withdraw account balance mismatch').eq(expectedBalance.toString());
    }

    it('Withdraw ERC20 success', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount.mul(2));
        await sendERC20.wait();
        const token = tokenContract.address;
        const tokenId = await ethProxy.resolveTokenId(token);

        // test one withdrawal
        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        const onchainBalBefore_first_subtest = await onchainBalance(wallet, tokenContract.address);

        await performWithdrawNoCheckContractBalance(
            wallet,
            tokenContract.address,
            tokenId,
            withdrawAmount.sub(FEE_AMOUNT)
        );
        expect(await zksyncContract.getPendingBalance(wallet.address, token)).eq('0');
        expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq(
            withdrawAmount
        );

        const onchainBalAfter_first_subtest = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter_first_subtest.sub(onchainBalBefore_first_subtest)).eq(
            withdrawAmount.sub(FEE_AMOUNT).toString()
        );

        // test two withdrawals
        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        const onchainBalBefore_second_subtest = await onchainBalance(wallet, tokenContract.address);

        await performWithdrawNoCheckContractBalance(
            wallet,
            tokenContract.address,
            tokenId,
            withdrawAmount.div(2).sub(FEE_AMOUNT)
        );
        expect(await zksyncContract.getPendingBalance(wallet.address, token)).eq(withdrawAmount.div(2).toString());
        expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq(
            withdrawAmount.div(2).toString()
        );
        await performWithdrawNoCheckContractBalance(
            wallet,
            tokenContract.address,
            tokenId,
            withdrawAmount.div(2).sub(FEE_AMOUNT)
        );
        expect(await zksyncContract.getPendingBalance(wallet.address, token)).eq('0');
        expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq('0');

        const onchainBalAfter_second_subtest = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter_second_subtest.sub(onchainBalBefore_second_subtest)).eq(
            withdrawAmount.sub(2 * FEE_AMOUNT).toString()
        );
    });

    it('Complete pending withdawals => should not complete transfer because of token fee', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        await tokenContract.transfer(zksyncContract.address, withdrawAmount);

        for (const tokenAddress of [tokenContract.address]) {
            const tokenId = await ethProxy.resolveTokenId(tokenAddress);

            await zksyncContract.setBalanceToWithdraw(exitWallet.address, tokenId, 0);

            const onchainBalBefore = await onchainBalance(exitWallet, tokenAddress);

            await zksyncContract.withdrawOrStoreExternal(tokenId, exitWallet.address, withdrawAmount);

            const onchainBalAfter = await onchainBalance(exitWallet, tokenAddress);

            expect(onchainBalAfter).eq(onchainBalBefore);

            expect(await zksyncContract.getPendingBalance(exitWallet.address, tokenAddress)).eq(withdrawAmount);

            // contract balance should not change
            expect(await onchainTokenBalanceOfContract(wallet, zksyncContract.address, tokenContract.address)).eq(
                withdrawAmount
            );
        }
    });
});

describe('zkSync process tokens which take fee from recipient', function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    before(async () => {
        [wallet, exitWallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncWithdrawalUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncWithdrawalUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractDeployFactory = await hardhat.ethers.getContractFactory('MintableERC20FeeAndPayoutTest');
        tokenContract = await tokenContractDeployFactory.deploy(true, false);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);
    });

    it('Make a deposit of tokens that should take a fee from recipient contract', async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther('1.0');
        await tokenContract.approve(zksyncContract.address, depositAmount);

        await zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address);
    });
});

describe('zkSync process tokens which adds payout to the recipient', function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    before(async () => {
        [wallet, exitWallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncWithdrawalUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncWithdrawalUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractDeployFactory = await hardhat.ethers.getContractFactory('MintableERC20FeeAndPayoutTest');
        tokenContract = await tokenContractDeployFactory.deploy(false, false);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);
    });

    it('Make a deposit of tokens that should adds payout to the recipient', async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther('1.0');
        await tokenContract.approve(zksyncContract.address, depositAmount);

        await zksyncContract.depositERC20(tokenContract.address, depositAmount, wallet.address);
    });
});
