<template>
<b-row>
    <b-col sm="6" class="col-xl-4 col-lg-5 col-md-6 col-sm-12 mb-5">
        <Alert v-bind:message="message">asd</Alert>
        <BalancesList balanceListId="onchain" v-bind:balances="onchainBalances"></BalancesList>
        <DepositButtons 
            componentId="onchain"
            v-bind:topBalances="onchainBalances" 
            v-bind:bottomBalances="contractBalances"
            v-on:depositEvent="depositOnchain"
            v-on:withdrawEvent="withdrawOnchain"
        ></DepositButtons>
        <BalancesList balanceListId="contract" v-bind:balances="contractBalances"></BalancesList>
        <DepositButtons 
            componentId="offchain"
            v-bind:topBalances="contractBalances" 
            v-bind:bottomBalances="franklinBalances"
            v-on:depositEvent="depositOffchain"
            v-on:withdrawEvent="withdrawOffchain"
        ></DepositButtons>
        <BalancesList balanceListId="franklin" v-bind:balances="franklinBalances"></BalancesList>
    </b-col>
    <b-col sm="6" class="col-xl-4 col-lg-5 col-md-6 col-sm-12 mb-5">
        else
    </b-col>
</b-row>
</template>

<script>
// TODO: remove this imports
import { ethers } from 'ethers'
import { Wallet, FranklinProvider } from 'franklin_lib'
import { WalletDecorator } from '../WalletDecorator'
// END-TODO

import Alert from '../components/Alert.vue'
import BalancesList from '../components/BalancesList.vue'
import DepositButtons from '../components/DepositButtons.vue'

const components = {
    Alert,
    BalancesList,
    DepositButtons
};

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export default {
    name: 'wallet',
    data: () => ({
        message: 'nn',
        onchainBalances: [],
        contractBalances: [],
        franklinBalances: [],
    }),
    async created() {
        // TODO: delete next block of code
        let franklinProvider = new FranklinProvider('http://localhost:3000', '0xc56E79CAA94C96DE01eF36560ac215cC7A4F0F47');
        // let signer = ethersProvider.getSigner();
        let provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
        window.signer = ethers.Wallet.fromMnemonic("fine music test violin matrix prize squirrel panther purchase material script deal", "m/44'/60'/0'/0/1").connect(provider);
        window.wallet = await Wallet.fromEthWallet(signer, franklinProvider);
        window.walletDecorator = new WalletDecorator(window.wallet);

        this.updateAccountInfo();
    },
    methods: {
        displayAlert(msg) {
            this.message = String(msg);
            alert(msg);
        },
        async depositOnchain(kwargs) {
            console.log('depositOnchain', kwargs);
            try {
                if ( ! window.wallet) {
                    this.$emit('alert', `Wallet is ${window.wallet}`);
                    return;
                }

                await window.walletDecorator.depositOnchain(kwargs);

                this.displayAlert(`deposit succeeded or something`);
                this.$emit('alert', `deposit succeeded or something`);
            } catch (e) {
                this.$emit('alert', `unknown error: ${e}`);
                this.displayAlert(`unknown error: ${e}`);
            }
        },
        async withdrawOnchain(kwargs) {
            console.log('withdrawOnchain', kwargs);
        },
        async depositOffchain(kwargs) {
            this.displayAlert(`depositOffchain ${JSON.stringify(kwargs)}`);
            try {
                await window.walletDecorator.depositOffchain(kwargs);

                this.displayAlert(`deposit succeeded or something`);
            } catch (e) {
                this.displayAlert(`unknown error: ${e}`);
            }
        },
        async withdrawOffchain(kwargs) {
            this.displayAlert(`withdrawOffchain ${JSON.stringify(kwargs)}`);
        },
        async updateAccountInfo() {
            await window.walletDecorator.updateState();
            this.onchainBalances = window.walletDecorator.onchainBalancesAsRenderableList();
            this.contractBalances = window.walletDecorator.contractBalancesAsRenderableList();
            this.franklinBalances = window.walletDecorator.franklinBalancesAsRenderableList();
            await sleep(2000);
            this.updateAccountInfo();
        }
    },
    components
}
</script>
