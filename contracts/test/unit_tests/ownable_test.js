const { expect } = require('chai');
const { getCallRevertReason } = require('./common');
const hardhat = require('hardhat');

describe('Ownable unit tests', function () {
    this.timeout(50000);

    let testContract;
    let deployer;
    let wallet;
    before(async () => {
        [deployer, wallet] = await hardhat.ethers.getSigners();
        const contractFactory = await hardhat.ethers.getContractFactory('Ownable');
        testContract = await contractFactory.deploy(deployer.address);
    });

    it('checking correctness of setting mastership in constructor', async () => {
        expect(await testContract.getMaster()).to.equal(deployer.address);
    });

    it('checking correctness of transferring mastership to zero address', async () => {
        let { revertReason } = await getCallRevertReason(() =>
            testContract.transferMastership('0x0000000000000000000000000000000000000000', { gasLimit: '300000' })
        );
        expect(revertReason).equal('1d');
    });

    it('checking correctness of transferring mastership', async () => {
        /// transfer mastership to wallet
        await testContract.transferMastership(wallet.address);
        expect(await testContract.getMaster()).to.equal(wallet.address);

        /// try to transfer mastership to deployer by deployer call
        let { revertReason } = await getCallRevertReason(() =>
            testContract.transferMastership(deployer.address, { gasLimit: '300000' })
        );
        expect(revertReason).equal('1c');

        /// transfer mastership back to deployer
        let testContract_with_wallet2_signer = await testContract.connect(wallet);
        await testContract_with_wallet2_signer.transferMastership(deployer.address);
        expect(await testContract.getMaster()).to.equal(deployer.address);
    });
});
