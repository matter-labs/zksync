import { txInfo, accountInfo, apiServer } from '../src/commands';
import 'isomorphic-fetch';
import type { Network } from '../src/common';
import { expect, use } from 'chai';
import chaiAsPromised from 'chai-as-promised';
import * as ethers from 'ethers';
import * as zksync from 'zksync';

use(chaiAsPromised);

describe('Tests', () => {
    const NETWORK = 'localhost';
    let alice: ethers.Wallet;
    let bob: ethers.Wallet;
    let txHash: string;

    before('make some deposits & transactions', async () => {
        const ethProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
        const syncProvider = await zksync.getDefaultProvider(NETWORK, 'HTTP');
        const ethWallet = ethers.Wallet.fromMnemonic(
            process.env.TEST_MNEMONIC as string,
            "m/44'/60'/0'/0/0"
        ).connect(ethProvider);
        alice = ethers.Wallet.createRandom().connect(ethProvider);
        bob = ethers.Wallet.createRandom();
        const [aliceWallet, syncWallet] = await Promise.all([
            zksync.Wallet.fromEthSigner(alice, syncProvider),
            zksync.Wallet.fromEthSigner(ethWallet, syncProvider)
        ]);
        const ethDeposit = await syncWallet.depositToSyncFromEthereum({
            depositTo: alice.address,
            token: 'ETH',
            amount: ethers.utils.parseEther('1.5')
        });
        const daiDeposit = await syncWallet.depositToSyncFromEthereum({
            depositTo: alice.address,
            token: 'DAI',
            amount: syncProvider.tokenSet.parseToken('DAI', '18.0'),
            approveDepositAmountForERC20: true
        });
        await Promise.all([ethDeposit.awaitReceipt(), daiDeposit.awaitReceipt()]);
        const changePubkey = await aliceWallet.setSigningKey();
        await changePubkey.awaitReceipt();
        const txHandle = await aliceWallet.syncTransfer({
            to: bob.address,
            token: 'ETH',
            amount: ethers.utils.parseEther('0.7')
        });
        await txHandle.awaitReceipt();
        const txInfo = await fetch(`${apiServer(NETWORK)}/account/${alice.address}/history/0/1`);
        txHash = (await txInfo.json())[0].hash;
        syncProvider.disconnect();
    });

    describe('Account Info', () => {
        it('should fetch correct info', async () => {
            const info = await accountInfo(alice.address, NETWORK);
            expect(info.address).to.equal(alice.address);
            expect(info.network).to.equal(NETWORK);
            expect(info.nonce).to.equal(2);
            expect(info.balances).to.have.property('DAI', '18.0');
            expect(info.balances.ETH).to.exist;
            expect(+info.balances.ETH).to.be.within(0.79, 0.8);
            expect(info.account_id).to.be.a('number');
        });

        it('should fail on invalid network', () => {
            const invalid_network = 'random' as Network;
            expect(accountInfo(alice.address, invalid_network)).to.be.rejected;
        });

        it('should be empty for non-existent account', async () => {
            const non_existent = ethers.Wallet.createRandom().address;
            const info = await accountInfo(non_existent, NETWORK);
            expect(info.balances).to.be.empty;
            expect(info.account_id).to.be.null;
        });
    });

    describe('Transaction Info', () => {
        it('should fetch correct info', async () => {
            const info = await txInfo(txHash, NETWORK);
            const tx = info.transaction;
            expect(info.network).to.be.equal(NETWORK);
            expect(tx?.status).to.be.equal('success');
            expect(tx?.hash).to.be.equal(txHash);
            expect(tx?.operation).to.be.equal('Transfer');
            expect(tx?.from).to.be.equal(alice.address.toLowerCase());
            expect(tx?.to).to.be.equal(bob.address.toLowerCase());
            expect(tx?.token).to.be.equal('ETH');
            expect(tx?.amount).to.be.equal('0.7');
            expect(tx?.nonce).to.be.equal(1);
            expect(tx?.fee).to.exist;
        });

        it('should fail on invalid network', () => {
            const invalid_network = 'random' as Network;
            expect(txInfo(txHash, invalid_network)).to.be.rejected;
        });

        it('should be empty for non-existent transaction', async () => {
            const non_existent =
                'sync-tx:8888888888888888888888888888888888888888888888888888888888888888';
            const info = await txInfo(non_existent, NETWORK);
            expect(info.transaction).to.be.null;
        });
    });
});
