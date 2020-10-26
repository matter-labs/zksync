import { expect, use } from 'chai';
import chaiAsPromised from 'chai-as-promised';
import fs from 'fs';
import mock from 'mock-fs';
import type { Network, Config } from '../src/types';
import * as ethers from 'ethers';
import * as zksync from 'zksync';
import * as commands from '../src/commands';
import { saveConfig, loadConfig, configLocation, DEFAULT_CONFIG } from '../src/config';

use(chaiAsPromised);

describe('Fetching Information', () => {
    let ethDepositor: string;
    let alice: ethers.Wallet;
    let bob: ethers.Wallet;
    let deposit_hash: string;
    let setkey_hash: string;
    let transfer_hash: string;

    before('make some deposits & transactions', async () => {
        const ethProvider = new ethers.providers.JsonRpcProvider();
        const syncProvider = await zksync.getDefaultProvider('localhost', 'HTTP');
        const ethWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC as string, "m/44'/60'/0'/0/0").connect(
            ethProvider
        );
        ethDepositor = ethWallet.address;
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
        const changePubkey = await aliceWallet.setSigningKey({
            feeToken: 'ETH'
        });
        await changePubkey.awaitReceipt();
        const txHandle = await aliceWallet.syncTransfer({
            to: bob.address,
            token: 'ETH',
            amount: ethers.utils.parseEther('0.7')
        });
        await txHandle.awaitReceipt();
        await syncProvider.disconnect();
        deposit_hash = daiDeposit.ethTx.hash;
        setkey_hash = changePubkey.txHash;
        transfer_hash = txHandle.txHash;
    });

    describe('Account Info', () => {
        it('should fetch correct info', async () => {
            const info = await commands.accountInfo(alice.address);
            expect(info.address).to.equal(alice.address);
            expect(info.network).to.equal('localhost');
            expect(info.nonce).to.equal(2);
            expect(info.balances).to.have.property('DAI', '18.0');
            expect(info.balances.ETH).to.exist;
            expect(+info.balances.ETH).to.be.within(0.77, 0.8);
            expect(info.account_id).to.be.a('number');
        });

        it('should fail on invalid network', () => {
            const invalid_network = 'random' as Network;
            expect(commands.accountInfo(alice.address, invalid_network)).to.be.rejected;
        });

        it('should be empty for non-existent account', async () => {
            const non_existent = ethers.Wallet.createRandom().address;
            const info = await commands.accountInfo(non_existent);
            expect(info.balances).to.be.empty;
            expect(info.account_id).to.be.null;
            expect(info.nonce).to.equal(0);
        });
    });

    describe('Transaction Info', () => {
        it('should fetch correct info - transfer', async () => {
            const info = await commands.txInfo(transfer_hash);
            const tx = info.transaction;
            expect(info.network).to.equal('localhost');
            expect(tx?.status).to.equal('success');
            expect(tx?.hash).to.equal(transfer_hash);
            expect(tx?.operation).to.equal('Transfer');
            expect(tx?.from).to.equal(alice.address.toLowerCase());
            expect(tx?.to).to.equal(bob.address.toLowerCase());
            expect(tx?.nonce).to.equal(1);
            expect(tx?.token).to.equal('ETH');
            expect(tx?.amount).to.equal('0.7');
            expect(tx?.fee).to.exist;
        });

        it('should fetch correct info - setkey', async () => {
            const info = await commands.txInfo(setkey_hash);
            const tx = info.transaction;
            expect(info.network).to.equal('localhost');
            expect(tx?.status).to.equal('success');
            expect(tx?.hash).to.equal(setkey_hash);
            expect(tx?.operation).to.equal('ChangePubKey');
            expect(tx?.from).to.equal(alice.address.toLowerCase());
            expect(tx?.to).to.be.a('string');
            expect(tx?.nonce).to.equal(0);
            expect(tx?.token).to.equal('ETH');
            expect(tx?.amount).to.not.exist;
            expect(tx?.fee).to.exist;
        });

        it('should fetch correct info - deposit', async () => {
            const info = await commands.txInfo(deposit_hash);
            const tx = info.transaction;
            expect(info.network).to.equal('localhost');
            expect(tx?.status).to.equal('success');
            expect(tx?.hash).to.equal(deposit_hash);
            expect(tx?.operation).to.equal('Deposit');
            expect(tx?.from).to.equal(ethDepositor.toLowerCase());
            expect(tx?.to).to.equal(alice.address.toLowerCase());
            expect(tx?.nonce).to.equal(-1);
            expect(tx?.token).to.equal('DAI');
            expect(tx?.amount).to.equal('18.0');
            expect(tx?.fee).to.not.exist;
        });

        it('should fail on invalid network', () => {
            const invalid_network = 'random' as Network;
            expect(commands.txInfo(transfer_hash, invalid_network)).to.be.rejected;
        });

        it('should be empty for non-existent transaction', async () => {
            const non_existent = 'sync-tx:8888888888888888888888888888888888888888888888888888888888888888';
            const info = await commands.txInfo(non_existent);
            expect(info.transaction).to.be.null;
        });
    });

    describe('Networks', () => {
        it('should return available networks', async () => {
            const networks = await commands.availableNetworks();
            expect(networks).to.be.an('array');
            expect(networks.length).to.be.within(1, 4);
            expect(networks).to.include('localhost');
        });
    });
});

describe('Config Management', () => {
    const alice = ethers.Wallet.createRandom();
    const bob = ethers.Wallet.createRandom();
    const eve = ethers.Wallet.createRandom();
    const config1: Config = {
        network: 'ropsten',
        defaultWallet: alice.address.toLowerCase(),
        wallets: {
            [alice.address.toLowerCase()]: alice.privateKey,
            [bob.address.toLowerCase()]: bob.privateKey
        }
    };
    const config2: Config = {
        network: 'mainnet',
        defaultWallet: eve.address.toLowerCase(),
        wallets: { [eve.address.toLowerCase()]: eve.privateKey }
    };

    beforeEach('create mock fs', () => {
        mock({
            '.zcli-config.json': JSON.stringify(config1),
            'customPath/.zcli-config.json': JSON.stringify(config1)
        });
    });

    afterEach('restore fs', () => {
        mock.restore();
    });

    it('should properly locate config file', () => {
        expect(configLocation()).to.equal('./.zcli-config.json');
        process.env.ZCLI_HOME = 'customPath';
        expect(configLocation()).to.equal('./.zcli-config.json');
        fs.unlinkSync('./.zcli-config.json');
        expect(configLocation()).to.equal('customPath/.zcli-config.json');
        fs.unlinkSync('customPath/.zcli-config.json');
        expect(configLocation()).to.equal('customPath/.zcli-config.json');
        delete process.env.ZCLI_HOME;
        expect(configLocation()).to.equal('./.zcli-config.json');
    });

    it('should properly read config file', () => {
        expect(loadConfig()).to.deep.equal(config1);
    });

    it('should create default config when needed', () => {
        fs.unlinkSync('./.zcli-config.json');
        expect(loadConfig()).to.deep.equal(DEFAULT_CONFIG);
    });

    it('should properly save config file', () => {
        saveConfig(config2);
        expect(loadConfig()).to.deep.equal(config2);
    });

    describe('Networks', () => {
        it('should properly get/set default network', () => {
            const config = loadConfig();
            const invalid_network = 'random' as Network;
            expect(commands.defaultNetwork(config)).to.equal(config1.network);
            expect(commands.defaultNetwork(config, 'rinkeby')).to.equal('rinkeby').to.equal(config.network);
            expect(() => commands.defaultNetwork(config, invalid_network)).to.throw();
        });
    });

    describe('Wallets', () => {
        it('should properly get/set default wallet', () => {
            const config = loadConfig();
            const new_wallet = bob.address;
            const invalid_wallet = '0x8888888888888888888888888888888888888888';
            expect(commands.defaultWallet(config)).to.equal(config1.defaultWallet);
            expect(commands.defaultWallet(config, new_wallet))
                .to.equal(new_wallet.toLowerCase())
                .to.equal(config.defaultWallet);
            expect(() => commands.defaultWallet(config, invalid_wallet)).to.throw();
        });

        it('should properly add a wallet', () => {
            const config = loadConfig();
            const wallet = ethers.Wallet.createRandom();
            expect(commands.addWallet(config, wallet.privateKey)).to.equal(wallet.address);
            expect(config.wallets).to.have.property(wallet.address.toLowerCase(), wallet.privateKey);
        });

        it('should properly remove a wallet', () => {
            const config = loadConfig();
            commands.removeWallet(config, alice.address);
            expect(config.wallets).to.not.have.key(alice.address.toLowerCase());
            expect(config.defaultWallet).to.be.null;
        });

        it('should list all wallets', () => {
            const config = loadConfig();
            const wallet_list = commands.listWallets(config);
            expect(wallet_list).to.be.an('array').with.lengthOf(2);
            expect(wallet_list).to.include(config.defaultWallet);
            expect(wallet_list).to.include(bob.address.toLowerCase());
        });
    });
});

describe('Making Transactions', () => {
    const rich = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC as string, "m/44'/60'/0'/0/0");
    const poor1 = ethers.Wallet.createRandom();
    const poor2 = ethers.Wallet.createRandom();

    it('should make a deposit - ETH', async () => {
        const hash = await commands.deposit({
            to: poor1.address,
            privkey: rich.privateKey,
            token: 'ETH',
            amount: '3.1415'
        });
        const info = await commands.txInfo(hash);
        expect(info).to.deep.equal({
            network: 'localhost',
            transaction: {
                status: 'success',
                from: rich.address.toLowerCase(),
                to: poor1.address.toLowerCase(),
                hash,
                operation: 'Deposit',
                nonce: -1,
                amount: '3.1415',
                token: 'ETH'
            }
        });
    });

    it('should make a deposit - DAI', async () => {
        const hash = await commands.deposit({
            to: poor1.address,
            privkey: rich.privateKey,
            token: 'DAI',
            amount: '2.7182'
        });
        const info = await commands.txInfo(hash);
        expect(info).to.deep.equal({
            network: 'localhost',
            transaction: {
                status: 'success',
                from: rich.address.toLowerCase(),
                to: poor1.address.toLowerCase(),
                hash,
                operation: 'Deposit',
                nonce: -1,
                amount: '2.7182',
                token: 'DAI'
            }
        });
    });

    it('should transfer tokens', async () => {
        await commands.deposit({
            to: poor1.address,
            privkey: rich.privateKey,
            token: 'DAI',
            amount: '1.4142'
        });
        const hash = await commands.transfer({
            to: poor2.address,
            privkey: poor1.privateKey,
            token: 'DAI',
            amount: '1.0'
        });
        const info = await commands.txInfo(hash);
        const tx = info.transaction;
        expect(info.network).to.equal('localhost');
        expect(tx?.status).to.equal('success');
        expect(tx?.from).to.equal(poor1.address.toLowerCase());
        expect(tx?.to).to.equal(poor2.address.toLowerCase());
        expect(tx?.hash).to.equal(hash);
        expect(tx?.operation).to.equal('Transfer');
        expect(tx?.amount).to.equal('1.0');
        expect(tx?.token).to.equal('DAI');
        expect(tx?.nonce).to.equal(1);
        expect(tx?.fee).to.exist;
        const account = await commands.accountInfo(poor2.address);
        expect(account.address).to.equal(poor2.address);
        expect(account.balances.DAI).to.equal('1.0');
    });

    it('should fail if not enough tokens', () => {
        // prettier-ignore
        expect(commands.transfer({
            to: poor2.address,
            privkey: poor1.privateKey,
            token: 'MLTT',
            amount: '73.0'
        })).to.be.rejected;
    });

    it('should wait for commitment', async () => {
        await commands.deposit({
            to: poor1.address,
            privkey: rich.privateKey,
            token: 'DAI',
            amount: '1.5'
        });
        const hash = await commands.transfer(
            {
                to: poor2.address,
                privkey: poor1.privateKey,
                token: 'DAI',
                amount: '1.0'
            },
            true
        );
        const info = await commands.txInfo(hash, 'localhost', 'COMMIT');
        expect(info.transaction).to.not.be.null;
    });
});
