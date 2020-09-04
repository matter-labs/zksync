import fetchMock from 'fetch-mock';
import { txInfo, accountInfo } from '../src/commands';
import type { Network } from '../src/common';
import { expect, use } from 'chai';
import chaiAsPromised from 'chai-as-promised';
const faker = require('faker');

use(chaiAsPromised);

describe('Transaction Info', () => {
    const tx_hash = 'sync-tx:2815e8d00d962a34b16032e9c5859454429fad5aafd0ed68fe46276f7dd2deb1';
    const fake_tx = {
        tx_type: faker.finance.transactionType(),
        from: faker.finance.ethereumAddress(),
        to: faker.finance.ethereumAddress(),
        token: 8,
        amount: '123400000000000000',
        fee: '180000000000000',
        block_number: faker.random.number(1000),
        nonce: faker.random.number(1000),
        created_at: faker.date.past(),
        fail_reason: null,
        tx: {
            /* does not matter */
        }
    };

    afterEach('uninstall mock fetch', () => {
        fetchMock.reset();
    });

    it('should query the right url', async () => {
        fetchMock.getOnce(/.*api\.zksync\.io.*/, fake_tx);
        await txInfo(tx_hash, 'ropsten');
        expect(fetchMock.done()).to.be.true;
        expect(fetchMock.lastUrl()).to.equal(
            `https://ropsten-api.zksync.io/api/v0.1/transactions_all/${tx_hash}`
        );
    });

    it('should format token amounts', async () => {
        fetchMock.getOnce(/.*api\.zksync\.io.*/, fake_tx);
        const info = await txInfo(tx_hash, 'ropsten');
        expect(info.transaction?.token).to.equal('BAT');
        expect(info.transaction?.amount).to.equal('0.1234');
        expect(info.transaction?.fee).to.equal('0.00018');
    });

    it('should get all properties', async () => {
        fetchMock.getOnce(/.*api\.zksync\.io.*/, fake_tx);
        const info = await txInfo(tx_hash, 'ropsten');
        const tx = info.transaction;
        expect(info.network).to.equal('ropsten');
        expect(tx?.from).to.equal(fake_tx.from);
        expect(tx?.to).to.equal(fake_tx.to);
        expect(tx?.operation).to.equal(fake_tx.tx_type.toLowerCase());
        expect(tx?.nonce).to.equal(fake_tx.nonce);
        expect(tx?.status).to.equal('success');
    });

    it('should account for non-existent transaction', async () => {
        fetchMock.getOnce(/.*api\.zksync\.io.*/, 'null');
        const info = await txInfo(tx_hash, 'ropsten');
        expect(info.transaction).to.equal(null);
    });
});

describe('Account Info', () => {
    const fake_address = faker.finance.ethereumAddress();

    it('should get all properties', async () => {
        const info = await accountInfo(fake_address, 'ropsten');
        expect(info.address).to.equal(fake_address);
        expect(info.network).to.equal('ropsten');
        expect(info.nonce).to.be.a('number');
        expect(info.balances).to.be.an('object');
        expect(info.account_id).to.satisfy((id: any) => id === null || typeof id === 'number');
    });

    it('should fail on invalid network', () => {
        const invalid_network = 'random' as Network;
        expect(accountInfo(fake_address, invalid_network)).to.be.rejected;
    });
});
