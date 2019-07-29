import { expect } from 'chai';
import BN = require('bn.js');
import { floatToInteger, integerToFloat } from '../src/utils';

describe('Packing and unpacking', function() {
    it('Test round-trip', function() {
        let initial_fee = new BN('12000000000');
        let packed_fee = integerToFloat(initial_fee, 4, 4, 10);
        console.log('Fee: ', initial_fee.toString(), ' ', packed_fee);
        let unpacked_fee = floatToInteger(packed_fee, 4, 4, 10);
        expect(initial_fee.eq(unpacked_fee)).to.equal(true);

        let initial_amount = new BN('987650000000000000000');
        let packed_amount = integerToFloat(initial_amount, 5, 19, 10);
        console.log('Amount: ', initial_amount.toString(), ' ', packed_amount);
        let unpacked_amount = floatToInteger(packed_amount, 5, 19, 10);
        expect(initial_amount.eq(unpacked_amount)).to.equal(true);
    });
});
