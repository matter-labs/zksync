import { expect } from 'chai';
import BN = require('bn.js');
import { floatToInteger, integerToFloat } from '../src/utils';

describe('Packing and unpacking', function() {
    it('Test round-trip', function() {
        let initial = new BN('12340000000000');
        let packed = integerToFloat(initial, 4, 4, 10);
        let unpacked = floatToInteger(packed, 4, 4, 10);
        expect(initial.eq(unpacked));
    });

    it('Print values', function() {
        let amount = new BN('12340000000000000000000');
        let packed_amount = integerToFloat(amount, 9, 15, 10);
        console.log(packed_amount);
        let fee = new BN('456000');
        let packed_fee = integerToFloat(fee, 4, 4, 10);
        console.log(packed_fee);
    });
});
