import { expect } from 'chai';
import BN = require('bn.js');
import { floatToInteger, integerToFloat } from '../src/utils';
import { pedersenHash } from '../src/crypto';

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

describe('Pedersen hash', function() {
    it('Test empty input', function() {
        let input = Buffer.from(new Array(0));
        let [expectedX, expectedY] = [
            new BN('154c12f40dfb646f34834f466a4532c2a6990e54cc92f83583c73c284828ae00', 'hex'),
            new BN('0e835ac588af244cc4195bcc936ae41c4d26605b50710e24bccb25055ebf1352', 'hex'),
        ];
        let resultPoint = pedersenHash(input);

        expect(resultPoint.getX().eq(expectedX)).equal(true);
        expect(resultPoint.getY().eq(expectedY)).equal(true);
    });

    it('Test known input of max size', function() {
        let input = Buffer.from(new Array(115).fill(144));
        let [expectedX, expectedY] = [
            new BN('1702aab88d54601a0aa5fa84b497513a7c7fb85a773c3b70fdc686a5b24ff9ac', 'hex'),
            new BN('2167673932490999f7eb7fd23f0b629ae7f16298bd1e93055a001682a7a0b064', 'hex'),
        ];
        let resultPoint = pedersenHash(input);

        expect(resultPoint.getX().eq(expectedX)).equal(true);
        expect(resultPoint.getY().eq(expectedY)).equal(true);
    });

    it('Test random, known input', function() {
        let input = Buffer.from([
            7,
            18,
            29,
            40,
            51,
            62,
            73,
            84,
            95,
            106,
            117,
            128,
            139,
            150,
            161,
            172,
            183,
            194,
            205,
            216,
            227,
            238,
            249,
            4,
            15,
            26,
            37,
            48,
            59,
            70,
            81,
            92,
            103,
            114,
            125,
            136,
            147,
            158,
            169,
            180,
            191,
            202,
            213,
            224,
            235,
            246,
            1,
            12,
            23,
            34,
            45,
            56,
            67,
            78,
            89,
            100,
            111,
            122,
            133,
            144,
            155,
            166,
            177,
            188,
            199,
            210,
            221,
            232,
            243,
            254,
            9,
            20,
            31,
            42,
            53,
            64,
            75,
            86,
            97,
            108,
            119,
            130,
            141,
            152,
            163,
            174,
            185,
            196,
            207,
            218,
            229,
            240,
            251,
            6,
            17,
            28,
            39,
        ]);
        let [expectedX, expectedY] = [
            new BN('21c40d5e70dfa538635620b94a8a3ccd9a021c922b9d1545c02f77d17599033a', 'hex'),
            new BN('10b076557ae7d728dd5bdf31d6ea9158d697d19a295e798c7b297634eadb28ad', 'hex'),
        ];
        let resultPoint = pedersenHash(input);

        expect(resultPoint.getX().eq(expectedX)).equal(true);
        expect(resultPoint.getY().eq(expectedY)).equal(true);
    });
});
