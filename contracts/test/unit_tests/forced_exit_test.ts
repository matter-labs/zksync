import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Signer, utils } from 'ethers';
import { ForcedExit } from '../../typechain/ForcedExit';
import { ForcedExitFactory } from '../../typechain/ForcedExitFactory';

import * as hardhat from 'hardhat';

const TX_AMOUNT = utils.parseEther('1.0');

use(solidity);

describe('ForcedExit unit tests', function () {
    this.timeout(50000);

    let forcedExitContract: ForcedExit;
    let wallet1: Signer;
    let wallet2: Signer;
    let wallet3: Signer;
    let wallet4: Signer;

    before(async () => {
        [wallet1, wallet2, wallet3, wallet4] = await hardhat.ethers.getSigners();

        const forcedExitContractFactory = await hardhat.ethers.getContractFactory('ForcedExit');
        const contract = await forcedExitContractFactory.deploy(wallet1.getAddress(), wallet2.getAddress());
        // Connecting the wallet to a potential receiver, who can withdraw the funds
        // on the master's behalf
        forcedExitContract = ForcedExitFactory.connect(contract.address, wallet2);
    });

    it('Check withdrawing fees', async () => {
        // The test checks the ability to withdraw the funds from the contract
        // after the user has sent them

        // Code style note: Could not use nested expects because
        // changeEtherBalance does not allow it

        // User sends funds to the contract
        const transferTxHandle = await wallet3.sendTransaction({
            to: forcedExitContract.address,
            value: TX_AMOUNT
        });
        // Check that the `FundsReceived` event was emitted
        expect(transferTxHandle).to.emit(forcedExitContract, 'FundsReceived').withArgs(TX_AMOUNT);

        // Withdrawing the funds from the contract to the wallet4
        const withdrawTxHandle = await forcedExitContract.withdrawPendingFunds(await wallet4.getAddress());

        // The pending funds have been received
        expect(withdrawTxHandle).to.changeEtherBalance(wallet4, TX_AMOUNT);
    });
});
