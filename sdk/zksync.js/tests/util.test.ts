import { expect } from 'chai';
import {
    closestPackableTransactionAmount,
    closestPackableTransactionFee,
    isTransactionAmountPackable,
    isTransactionFeePackable,
    TokenSet
} from '../src/utils';
import { BigNumber } from 'ethers';

describe('Packing and unpacking', function () {
    it('Test basic fee packing/unpacking', function () {
        let nums = ['0', '1', '2', '2047000', '1000000000000000000000000000000000'];
        for (let num of nums) {
            const bigNumberAmount = BigNumber.from(num);
            expect(closestPackableTransactionFee(bigNumberAmount).toString()).equal(
                bigNumberAmount.toString(),
                'fee packing'
            );
            expect(isTransactionAmountPackable(bigNumberAmount), 'check amount pack').eq(true);
            expect(closestPackableTransactionAmount(bigNumberAmount).toString()).equal(
                bigNumberAmount.toString(),
                'amount packing'
            );
            expect(isTransactionFeePackable(bigNumberAmount), 'check fee pack').eq(true);
        }
    });
});

describe('Token cache resolve', function () {
    it('Test token cache resolve', function () {
        const tokens = {
            ETH: {
                address: '0x0000000000000000000000000000000000000000',
                id: 0,
                symbol: 'ETH',
                decimals: 18
            },
            'ERC20-1': {
                address: '0xEEeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                id: 1,
                symbol: 'ERC20-1',
                decimals: 6
            }
        };
        const tokenCache = new TokenSet(tokens);

        expect(tokenCache.resolveTokenId('ETH')).eq(0, 'ETH by id resolve');
        expect(tokenCache.resolveTokenId('0x0000000000000000000000000000000000000000')).eq(0, 'ETH by addr resolve');
        expect(tokenCache.resolveTokenId('ERC20-1')).eq(1, 'ERC20 by id resolve');
        expect(tokenCache.resolveTokenId('0xEEeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee')).eq(1, 'ERC20 by addr resolve');
        expect(tokenCache.resolveTokenId('0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee')).eq(1, 'ERC20 by addr resolve');
        expect(() => tokenCache.resolveTokenId('0xdddddddddddddddddddddddddddddddddddddddd')).to.throw();
        expect(() => tokenCache.resolveTokenId('ERC20-2')).to.throw();
    });
});
