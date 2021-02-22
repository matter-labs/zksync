import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Signer, utils } from 'ethers';
import { ForcedExit } from '../../typechain/ForcedExit';
import { ForcedExitFactory } from '../../typechain/ForcedExitFactory';
import { SelfDestructFactory } from '../../typechain/SelfDestructFactory';

import * as hardhat from 'hardhat';

const TX_AMOUNT = utils.parseEther('1.0');

use(solidity);

describe('ForcedExit unit tests', function () {
    this.timeout(50000);

    let forcedExitContract: ForcedExit;
    let wallet1: Signer;
    let wallet2: Signer;
    let wallet3: Signer;

    before(async () => {
        [wallet1, wallet2, wallet3] = await hardhat.ethers.getSigners();

        const forcedExitContractFactory = await hardhat.ethers.getContractFactory('ForcedExit');
        const contract = await forcedExitContractFactory.deploy(wallet1.getAddress());
        forcedExitContract = ForcedExitFactory.connect(contract.address, wallet1);
    });

    it('Check redirecting funds to receiver', async () => {
        // The test checks that when users send funds to the contract
        // the funds will be redirected to the receiver address that is set
        // by the master of the ForcedExit contract

        // Setting receiver who will should get all the funds sent
        // to the contract
        await forcedExitContract.setReceiver(await wallet3.getAddress());

        // Could not use nested expects because
        // changeEtherBalance does not allow it

        // User sends tranasctions
        const txHandle = await wallet2.sendTransaction({
            to: forcedExitContract.address,
            value: TX_AMOUNT
        });
        // Check that the `FundsReceived` event was emitted
        expect(txHandle).to.emit(forcedExitContract, 'FundsReceived').withArgs(TX_AMOUNT);

        // The receiver received the balance
        expect(txHandle).to.changeEtherBalance(wallet3, TX_AMOUNT);
    });

    it('Check receiving pending funds', async () => {
        // The test checks that it is possible for the master of the contract
        // to withdraw funds that got stuck on the contract for some unknown reason.
        // One example is when another contract does selfdestruct and submits funds
        // to the ForcedExit contract.

        // Create the contract which will self-destruct itself
        const selfDestructContractFactory = await hardhat.ethers.getContractFactory('SelfDestruct');
        const contractDeployed = await selfDestructContractFactory.deploy();
        const selfDestructContract = SelfDestructFactory.connect(contractDeployed.address, contractDeployed.signer);

        // Supplying funds to the self-desctruct contract
        await wallet2.sendTransaction({
            to: selfDestructContract.address,
            value: TX_AMOUNT
        });

        // Destroying the self-destruct contract which sends TX_AMOUNT ether to the ForcedExit
        // contract which were not redirected to the receiver
        await selfDestructContract.connect(wallet2).destroy(forcedExitContract.address);

        // The master withdraws the funds and they should arrive to him
        expect(
            await forcedExitContract.withdrawPendingFunds(await wallet1.getAddress(), TX_AMOUNT)
        ).to.changeEtherBalance(wallet1, TX_AMOUNT);
    });

    it('Check disabling and enabling', async () => {
        // The test checks that disabling and enabling of the ForcedExit contract works.

        // Disabling transfers to the contract
        await forcedExitContract.disable();

        // The contract is disabled. Thus, transfering to it should fail
        expect(
            wallet2.sendTransaction({
                to: forcedExitContract.address,
                value: TX_AMOUNT
            })
        ).to.be.reverted;

        // Enabling transfers to the contract
        await forcedExitContract.enable();

        // The contract is enabled. Thus, transfering to it should not fail
        expect(
            wallet2.sendTransaction({
                to: forcedExitContract.address,
                value: TX_AMOUNT
            })
        ).to.not.be.reverted;
    });
});
