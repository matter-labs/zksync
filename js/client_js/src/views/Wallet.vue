<template>
<b-container>
    <header>
    <div class="collapse bg-dark" id="navbarHeader">
        <div class="container">
        <div class="row">
            <div class="col-sm-8 col-md-7 py-4">
            <h4 class="text-white">About</h4>
            <p class="text-muted">Add some information about the album below, the author, or any other background context. Make it a few sentences long so folks can pick up some informative tidbits. Then, link them off to some social networking sites or contact information.</p>
            </div>
            <div class="col-sm-4 offset-md-1 py-4">
            <h4 class="text-white">Contact</h4>
            <ul class="list-unstyled">
                <li><a href="#" class="text-white">Follow on Twitter</a></li>
                <li><a href="#" class="text-white">Like on Facebook</a></li>
                <li><a href="#" class="text-white">Email me</a></li>
            </ul>
            </div>
        </div>
        </div>
    </div>
    <div class="navbar navbar-dark bg-dark shadow-sm">
        <div class="container d-flex justify-content-between">
        <a href="#" class="navbar-brand d-flex align-items-center">
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" aria-hidden="true" class="mr-2" viewBox="0 0 24 24" focusable="false"><path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"/><circle cx="12" cy="13" r="4"/></svg>
            <strong>Album</strong>
        </a>
        <button class="navbar-toggler" type="button" data-toggle="collapse" data-target="#navbarHeader" aria-controls="navbarHeader" aria-expanded="false" aria-label="Toggle navigation">
            <span class="navbar-toggler-icon"></span>
        </button>
        </div>
    </div>
    </header>
    <b-row>
        <b-col xl="6">
            <b-container>
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
                    v-bind:depositFeeNeeded="true"
                    v-on:depositEvent="depositOffchain"
                    v-on:withdrawEvent="withdrawOffchain"
                ></DepositButtons>
                <BalancesList balanceListId="franklin" v-bind:balances="franklinBalances"></BalancesList>
            </b-container>
        </b-col>
        <b-col xl="6">
            else
        </b-col>
    </b-row>
</b-container>
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
