const { expect } = require('chai');
const { getCallRevertReason } = require('./common');
const hardhat = require('hardhat');

describe('Ownable unit tests', function () {
    this.timeout(50000);

    let testContract;
    let owner_wallet;
    let another_wallet;
    before(async () => {
        [owner_wallet, another_wallet] = await hardhat.ethers.getSigners();
        const contractFactory = await hardhat.ethers.getContractFactory('Ownable');
        testContract = await contractFactory.deploy(owner_wallet.address);
    });

    it('checking correctness of setting mastership in constructor', async () => {
        expect(await testContract.getMaster()).to.equal(owner_wallet.address);
    });

    it('checking correctness of transferring mastership to zero address', async () => {
        let { revertReason } = await getCallRevertReason(() =>
            testContract.transferMastership('0x0000000000000000000000000000000000000000', { gasLimit: '300000' })
        );
        expect(revertReason).equal('1d');
    });

    it('checking correctness of transferring mastership', async () => {
        /// transfer mastership to another_wallet
        await testContract.transferMastership(another_wallet.address);
        expect(await testContract.getMaster()).to.equal(another_wallet.address);

        /// try to transfer mastership to owner_wallet by owner_wallet call
        let { revertReason } = await getCallRevertReason(() =>
            testContract.transferMastership(owner_wallet.address, { gasLimit: '300000' })
        );
        expect(revertReason).equal('1c');

        /// transfer mastership back to owner_wallet
        let testContract_with_wallet2_signer = await testContract.connect(another_wallet);
        await testContract_with_wallet2_signer.transferMastership(owner_wallet.address);
        expect(await testContract.getMaster()).to.equal(owner_wallet.address);
    });
});
