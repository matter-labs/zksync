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
});
