import { expect } from 'chai';
import { Signer, Contract, ContractTransaction, utils, BigNumber } from 'ethers';
import * as hardhat from 'hardhat';

const TX_AMOUNT = utils.parseEther('1.0');

describe('ForcedExit unit tests', function () {
    this.timeout(50000);

    let forcedExitContract: Contract;
    let wallet1: Signer;
    let wallet2: Signer;
    let wallet3: Signer;

    before(async () => {
        [wallet1, wallet2, wallet3] = await hardhat.ethers.getSigners();

        const forcedExitContractFactory = await hardhat.ethers.getContractFactory('ForcedExit');
        forcedExitContract = await forcedExitContractFactory.deploy(wallet1.getAddress());
        forcedExitContract.connect(wallet1);
    });

    it('Check redirecting funds to receiver', async () => {
        const setReceiverHandle = await forcedExitContract.setReceiver(wallet3.getAddress());
        await setReceiverHandle.wait();

        const receiverBalanceBefore = await wallet3.getBalance();
        const txHandle = await wallet2.sendTransaction({
            to: forcedExitContract.address,
            value: TX_AMOUNT
        });
        const txReceipt = await txHandle.wait();

        expect(txReceipt.logs.length == 1, 'No events were emitted').to.be.true;
        const receivedFundsAmount: BigNumber = forcedExitContract.interface.parseLog(txReceipt.logs[0]).args[0];

        expect(receivedFundsAmount.eq(TX_AMOUNT), "Didn't emit the amount of sent data").to.be.true;
        const receiverBalanceAfter = await wallet3.getBalance();
        const diff = receiverBalanceAfter.sub(receiverBalanceBefore);
        expect(diff.eq(TX_AMOUNT), 'Funds were not redirected to the receiver').to.be.true;
    });

    it('Check receiving pending funds', async () => {
        const selfDestructContractFactory = await hardhat.ethers.getContractFactory('SelfDestruct');
        let selfDestructContract: Contract = await selfDestructContractFactory.deploy();

        const txHandle = await wallet2.sendTransaction({
            to: selfDestructContract.address,
            value: TX_AMOUNT
        });
        await txHandle.wait();
        selfDestructContract.connect(wallet2);

        const destructHandle: ContractTransaction = await selfDestructContract.destroy(forcedExitContract.address);
        await destructHandle.wait();
        const masterBalanceBefore = await wallet1.getBalance();

        const withdrawHandle: ContractTransaction = await forcedExitContract.withdrawPendingFunds(
            wallet1.getAddress(),
            TX_AMOUNT
        );
        const withdrawReceipt = await withdrawHandle.wait();
        const masterBalanceAfter = await wallet1.getBalance();

        const diff = masterBalanceAfter.sub(masterBalanceBefore);
        const expectedDiff = TX_AMOUNT.sub(withdrawReceipt.gasUsed.mul(withdrawHandle.gasPrice));
        expect(diff.eq(expectedDiff), 'Pending funds have not arrived to the account').to.be.true;
    });

    it('Check disabling and enabling', async () => {
        const disableHandle = await forcedExitContract.disable();
        await disableHandle.wait();

        let failed1 = false;
        try {
            const txHandle = await wallet2.sendTransaction({
                to: forcedExitContract.address,
                value: TX_AMOUNT
            });
            await txHandle.wait();
        } catch {
            failed1 = true;
        }

        expect(failed1, 'Transfer to the disabled contract does not fail').to.be.true;

        const enableHandle = await forcedExitContract.enable();
        await enableHandle.wait();

        const txHandle = await wallet2.sendTransaction({
            to: forcedExitContract.address,
            value: TX_AMOUNT
        });
        const txReceipt = await txHandle.wait();

        expect(txReceipt.blockNumber, 'A transfer to the enabled account have failed').to.exist;
    });
});
