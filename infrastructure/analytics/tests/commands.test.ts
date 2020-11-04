import chaiAsPromised from 'chai-as-promised';
import { expect, use } from 'chai';
import * as ethers from 'ethers';
import * as zksync from 'zksync';
import { Config, Network } from '../src/types';
import { loadConfig } from '../src/config';
import { TimePeriod } from '../src/utils';
import * as commands from '../src/commands';

use(chaiAsPromised);

describe('Tests', () => {
    const network = 'localhost';
    let config: Config;
    let validTimePeriod: TimePeriod;
    let invalidTimePeriod: TimePeriod;

    before('prepare auxiliary data & create new zksync account, make transfer', async () => {
        config = loadConfig(network);
        const timeFrom = new Date().toISOString();

        const ethProvider = new ethers.providers.JsonRpcProvider();
        const zksProvider = await zksync.getDefaultProvider(network, 'HTTP');

        const ethWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC as string, "m/44'/60'/0'/0/0").connect(
            ethProvider
        );

        const alice = ethers.Wallet.createRandom().connect(ethProvider);
        const bob = ethers.Wallet.createRandom();
        const [aliceWallet, syncWallet] = await Promise.all([
            zksync.Wallet.fromEthSigner(alice, zksProvider),
            zksync.Wallet.fromEthSigner(ethWallet, zksProvider)
        ]);

        const daiDeposit = await syncWallet.depositToSyncFromEthereum({
            depositTo: alice.address,
            token: 'DAI',
            amount: zksProvider.tokenSet.parseToken('DAI', '10.0'),
            approveDepositAmountForERC20: true
        });

        await daiDeposit.awaitReceipt();
        const changePubkey = await aliceWallet.setSigningKey({ feeToken: 'ETH' });
        await changePubkey.awaitReceipt();

        const txHandle = await aliceWallet.syncTransfer({
            to: bob.address,
            token: 'DAI',
            amount: zksProvider.tokenSet.parseToken('DAI', '0.1')
        });
        await txHandle.awaitVerifyReceipt();
        await zksProvider.disconnect();

        const timeTo = new Date().toISOString();
        validTimePeriod = new TimePeriod(timeFrom, timeTo);
        invalidTimePeriod = new TimePeriod(timeTo, timeFrom);
    });

    describe('Balances Info', () => {
        it('should be non-empty for operator account', async () => {
            const balances = await commands.currentBalances(network, config.operator_fee_address);
            expect(balances.total.eth).not.to.equal(0);
            expect(balances.total.usd).not.to.equal(0);

            expect(balances['DAI'].eth).not.to.equal(0);
            expect(balances['DAI'].usd).not.to.equal(0);
            expect(balances['DAI'].amount).not.to.equal(0);
        });

        it('should be empty for non-existent account', async () => {
            const non_existent = ethers.Wallet.createRandom().address;
            const balances = await commands.currentBalances(network, non_existent);
            expect(balances.total.eth).to.equal(0);
            expect(balances.total.usd).to.equal(0);

            expect(balances['DAI'].eth).to.equal(0);
            expect(balances['DAI'].usd).to.equal(0);
            expect(balances['DAI'].amount).to.equal(0);
        });

        it('should fail on invalid network', async () => {
            const invalidNetwork = 'invalid' as Network;
            expect(commands.currentBalances(invalidNetwork, config.operator_fee_address)).to.be.rejected;
        });
    });

    describe('Collected fees Info', () => {
        it('should be non-empty for correct REST API address', async () => {
            const fees = await commands.collectedFees(network, config.rest_api_address, validTimePeriod);
            expect(fees['spent by SENDER ACCOUNT'].eth).not.to.equal(0);
            expect(fees['spent by SENDER ACCOUNT'].usd).not.to.equal(0);

            expect(fees['collected fees'].total.eth).not.to.equal(0);
            expect(fees['collected fees'].total.usd).not.to.equal(0);
        });

        it('should fail on invalid network', async () => {
            const invalidNetwork = 'invalid' as Network;
            expect(commands.collectedFees(invalidNetwork, config.rest_api_address, validTimePeriod)).to.be.rejected;
        });

        it('should fail on invalid time period', async () => {
            expect(commands.collectedFees(network, config.rest_api_address, invalidTimePeriod)).to.be.rejected;
        });
    });

    describe('Liquidations Info', () => {
        it('should fail on invalid localhost', async () => {
            expect(
                commands.collectedTokenLiquidations(
                    network,
                    config.operator_fee_address,
                    validTimePeriod,
                    config.etherscan_api_key
                )
            ).to.be.rejected;
        });
    });
});
