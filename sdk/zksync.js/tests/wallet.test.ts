import { expect } from 'chai';
import { BigNumber, ethers } from 'ethers';
import { Wallet } from '../src/wallet';
import { Signer } from '../src/signer';
import { getTokens } from 'reading-tool';

import { Provider } from '../src/provider';
import { weiRatio, serializeOrder, MAX_TIMESTAMP } from '../src/utils';
import { Ratio } from '../src/types';
import { verifySignature } from '../src/crypto';

describe('Wallet with mock provider', function () {
    async function getWallet(ethPrivateKey: Uint8Array, network: string): Promise<Wallet> {
        const ethWallet = new ethers.Wallet(ethPrivateKey);
        const tokens = getTokens(network);
        const mockProvider = await Provider.newMockProvider(network, ethPrivateKey, () => [...tokens]);
        const wallet = await Wallet.fromEthSigner(ethWallet, mockProvider);
        return wallet;
    }

    it('Wallet has valid address', async function () {
        const key = new Uint8Array(new Array(32).fill(5));
        const wallet = await getWallet(key, 'mainnet');
        expect(wallet.address()).eq('0xd09Ad14080d4b257a819a4f579b8485Be88f086c', 'Wallet address does not match');
    });

    it("Wallet's account info has the same address as the wallet itself", async function () {
        const key = new Uint8Array(new Array(32).fill(10));
        const wallet = await getWallet(key, 'mainnet');
        const accountState = await wallet.getAccountState();
        expect(accountState.address).eq(wallet.address(), 'Wallet address does not match the accountState.address');
    });

    it('Wallet has defined account id', async function () {
        const key = new Uint8Array(new Array(32).fill(14));
        const wallet = await getWallet(key, 'mainnet');
        const accountId = await wallet.getAccountId();
        expect(accountId).eq(42, "Wallet's accountId does not match the hardcoded mock value");
    });

    it('Wallet has expected committed balances', async function () {
        const key = new Uint8Array(new Array(32).fill(40));
        const wallet = await getWallet(key, 'mainnet');
        const balance = await wallet.getBalance('DAI', 'committed');
        expect(balance).eql(
            BigNumber.from(12345),
            "Wallet's committed balance does not match the hardcoded mock value"
        );
    });

    it('Wallet do not have unexpected committed balances', async function () {
        const key = new Uint8Array(new Array(32).fill(40));
        const wallet = await getWallet(key, 'mainnet');

        let thrown = false;
        try {
            await wallet.getBalance('ETH', 'committed');
        } catch {
            thrown = true;
        }

        expect(thrown, 'getBalance call was expected to throw an exception').to.be.true;
    });

    it('Wallet has expected verified balances', async function () {
        const key = new Uint8Array(new Array(32).fill(50));
        const wallet = await getWallet(key, 'mainnet');
        const balance = await wallet.getBalance('USDC', 'verified');
        expect(balance).eql(
            BigNumber.from(98765),
            "Wallet's committed balance does not match the hardcoded mock value"
        );
    });

    it('Wallet do not have unexpected verified balances', async function () {
        const key = new Uint8Array(new Array(32).fill(50));
        const wallet = await getWallet(key, 'mainnet');

        let thrown = false;
        try {
            await wallet.getBalance('ETH', 'verified');
        } catch {
            thrown = true;
        }

        expect(thrown, 'getBalance call was expected to throw an exception').to.be.true;
    });

    it("Wallet's signing key checking", async function () {
        const key = new Uint8Array(new Array(32).fill(60));
        const wallet = await getWallet(key, 'mainnet');
        expect(await wallet.isSigningKeySet()).eq(true, "Wallet's signing key is unset");
    });

    it("Test signing signature", async function() {
        const key = new Uint8Array(new Array(32).fill(60));
        const signer = Signer.fromPrivateKey(key);

        const order = {
            accountId: 12,
            recipient: ethers.utils.hexlify(ethers.utils.randomBytes(20)),
            nonce: 13,
            tokenSell: 0,
            tokenBuy: 1,
            ratio: [1,2] as Ratio,
            amount: '10',
            validFrom: 0,
            validUntil: MAX_TIMESTAMP
        };

        const signedOrder = await signer.signSyncOrder(order);
        const orderSignedBytes = serializeOrder(order);
        const valid = await verifySignature(orderSignedBytes, signedOrder.signature!);

        console.log(valid);
    });
});
