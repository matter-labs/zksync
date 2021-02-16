import { expect } from 'chai';
import {
    closestGreaterOrEqPackableTransactionAmount,
    closestGreaterOrEqPackableTransactionFee,
    closestPackableTransactionAmount,
    closestPackableTransactionFee,
    isTransactionAmountPackable,
    isTransactionFeePackable,
    TokenSet,
    getTxHash
} from '../src/utils';
import { Transfer, ChangePubKey, Withdraw, ForcedExit } from '../src/types';
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
            expect(closestGreaterOrEqPackableTransactionFee(bigNumberAmount).toString()).equal(
                bigNumberAmount.toString(),
                'fee packing up'
            );
            expect(isTransactionAmountPackable(bigNumberAmount), 'check amount pack').eq(true);
            expect(closestPackableTransactionAmount(bigNumberAmount).toString()).equal(
                bigNumberAmount.toString(),
                'amount packing'
            );
            expect(closestGreaterOrEqPackableTransactionAmount(bigNumberAmount).toString()).equal(
                bigNumberAmount.toString(),
                'amount packing up'
            );
            expect(isTransactionFeePackable(bigNumberAmount), 'check fee pack').eq(true);
        }
        expect(closestPackableTransactionFee('2048').toString()).equal('2047', 'fee packing');
        expect(closestGreaterOrEqPackableTransactionFee('2048').toString()).equal('2050', 'fee packing up');
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

describe('Test getTxHash', function () {
    it('Test Transfer', async function () {
        const transfer = {
            type: 'Transfer',
            accountId: 123,
            from: '0xdddddddddddddddddddddddddddddddddddddddd',
            to: '0xeddddddddddddddddddddddddddddddddddddddd',
            token: 0,
            amount: 23,
            fee: 88,
            nonce: 123,
            validFrom: 12,
            validUntil: 1232321
        };
        const transferHash = getTxHash(transfer as Transfer);
        expect(
            'sync-tx:9aa2460771722dfc15fc371e11d8412b63acdd0a483b888336234fc4b825b00b' === transferHash,
            'Incorrect transfer hash'
        ).to.be.true;
    });
    it('Test Withdraw', async function () {
        const withdraw = {
            type: 'Withdraw',
            accountId: 1,
            from: '0xddddddddddddddddddddddddddddddddddddddde',
            to: '0xadddddddddddddddddddddddddddddddddddddde',
            token: 12,
            amount: '123',
            fee: '897',
            nonce: 1,
            validFrom: 90809,
            validUntil: 873712938
        };
        const withdrawHash = getTxHash(withdraw as Withdraw);
        expect(
            'sync-tx:84365ebb70259b8f6d6d9729e660f1ea9ecb2dbeeefd449bed54ac144d80a315' === withdrawHash,
            'Incorrect withdrawal hash'
        ).to.be.true;
    });
    it('Test ChangePubKey', async function () {
        const changePubKey = {
            type: 'ChangePubKey',
            accountId: 2,
            account: '0xaddddddddddddddddddddddddddddddddddddd0e',
            newPkHash: '0xadddddddd1234ddddddddddddddddddddddddd0e',
            feeToken: 20,
            fee: 98,
            nonce: 32,
            validFrom: 177,
            validUntil: 52443
        };
        const changePubKeyHash = getTxHash(changePubKey as ChangePubKey);
        expect(
            'sync-tx:486629437f43e9d9383431e2d075ba194d9e549c08b03db234ca4edaebb2200f' === changePubKeyHash,
            'Incorrect changePubKey hash'
        ).to.be.true;
    });
    it('Test ForcedExit', async function () {
        const forcedExit = {
            type: 'ForcedExit',
            initiatorAccountId: 776,
            target: '0xadddddddd1234ddddd777ddddddddddddddddd0e',
            token: 5,
            fee: 123,
            nonce: 5,
            validFrom: 8978,
            validUntil: 57382678
        };
        const forcedExitHash = getTxHash(forcedExit as ForcedExit);
        expect(
            'sync-tx:0f5cba03550d1ab984d6f478c79aeb6f6961873df7a5876c9af4502364163d03' === forcedExitHash,
            'Incorrect forcedExit hash'
        ).to.be.true;
    });
});
