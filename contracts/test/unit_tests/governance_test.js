const hardhat = require('hardhat');
const { expect } = require('chai');
const { getCallRevertReason } = require('./common');

describe('Governance unit tests', function () {
    this.timeout(50000);

    let testContract;
    before(async () => {
        const contractFactory = await hardhat.ethers.getContractFactory('TestGovernance');
        testContract = await contractFactory.deploy();
        await testContract.initialize(
            hardhat.ethers.utils.defaultAbiCoder.encode(['address'], [await testContract.signer.getAddress()])
        );
        await testContract.changeTokenGovernance(await testContract.signer.getAddress());
    });

    it('checking correctness of using MAX_AMOUNT_OF_REGISTERED_TOKENS constant', async () => {
        const MAX_AMOUNT_OF_REGISTERED_TOKENS = 5;
        for (let step = 1; step <= MAX_AMOUNT_OF_REGISTERED_TOKENS + 1; step++) {
            let { revertReason } = await getCallRevertReason(() =>
                testContract.addToken('0x' + step.toString().padStart(40, '0'))
            );
            if (step !== MAX_AMOUNT_OF_REGISTERED_TOKENS + 1) {
                expect(revertReason).equal('VM did not revert');
            } else {
                expect(revertReason).not.equal('VM did not revert');
            }
        }
    });
    it('Check correct register factory', async () => {
        const data = await testContract.publicPackRegisterNFTFactoryMsg(
            3,
            '0x5b51e2299151124ea4b8763c8c1a167740302681',
            '0x2dd77f58e193f7789a6aeb297ea4439c073c7f9c'
        );
        console.log(Buffer.from(data));
    });
});
