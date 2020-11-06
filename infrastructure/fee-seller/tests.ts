import { types } from 'zksync';
import { BigNumber, BigNumberish, utils } from 'ethers';
import { expect } from 'chai';
import { isOperationFeeAcceptable, numberAsFractionInBIPs } from './utils';

describe('Withdraw token', () => {
    it('numberAsFractionInBIPs', () => {
        expect(numberAsFractionInBIPs(5, 100).toNumber()).eq(500);
        expect(numberAsFractionInBIPs(2, 1).toNumber()).eq(20000);
        expect(() => numberAsFractionInBIPs('0.1', 1)).throw('INVALID_ARGUMENT');
        expect(() => numberAsFractionInBIPs(1, '0.1')).throw('INVALID_ARGUMENT');
        expect(() => numberAsFractionInBIPs(-1, 1)).throw('Numbers should be non-negative');
        expect(() => numberAsFractionInBIPs(-1, -1)).throw('Numbers should be non-negative');
        expect(() => numberAsFractionInBIPs(1, -1)).throw('Numbers should be non-negative');
        expect(() => numberAsFractionInBIPs(1, 0)).throw("Base fraction can't be 0");
        expect(numberAsFractionInBIPs(2, 1).toNumber()).eq(20000);

        const maxInt = BigNumber.from(Number.MAX_SAFE_INTEGER.toString());
        expect(numberAsFractionInBIPs(maxInt.mul(4), maxInt.mul(2)).toNumber()).eq(20000);
    });

    it('isWithdrawRequired', () => {
        expect(isOperationFeeAcceptable(utils.parseEther('100.0'), utils.parseEther('1.0'), 1)).eq(true);
        expect(isOperationFeeAcceptable(utils.parseEther('100.0'), utils.parseEther('1.0'), 2)).eq(true);
        expect(isOperationFeeAcceptable(utils.parseEther('200.0'), utils.parseEther('5.0'), 2)).eq(false);
        expect(isOperationFeeAcceptable('0', BigNumber.from(100), 2)).eq(false);
    });
});
