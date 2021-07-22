const hardhat = require('hardhat');
const { BigNumber } = require('ethers');
const { expect } = require('chai');
const { getCallRevertReason } = require('./common');

describe('Bytes unit tests', function () {
    this.timeout(50000);

    let bytesTestContract;
    before(async () => {
        const contractFactory = await hardhat.ethers.getContractFactory('BytesTest');
        bytesTestContract = await contractFactory.deploy();
    });

    // read

    it('should read bytes', async () => {
        let r = await bytesTestContract.read('0x0102030405060708', 4, 2);
        expect(r.data).equal('0x0506');
        expect(r.newOffset).equal(BigNumber.from(6));
    });

    it('should fail to read bytes beyond range', async () => {
        let { revertReason } = await getCallRevertReason(() => bytesTestContract.read('0x0102030405060708', 8, 2));
        expect(revertReason).equal('Z');
    });

    it('should fail to read too many bytes', async () => {
        let { revertReason } = await getCallRevertReason(() => bytesTestContract.read('0x0102030405060708', 4, 5));
        expect(revertReason).equal('Z');
    });

    // types

    it('should convert uint24', async () => {
        const x = 0x010203;
        let r = await bytesTestContract.testUInt24(x);
        expect(x).equal(r.r);
        expect(r.offset).equal(3);
    });
});
