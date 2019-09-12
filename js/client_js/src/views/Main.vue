<template>
    <b-container>
        <header>
            <nav class="navbar navbar-expand-md navbar-dark bg-dark mb-4">
            <a class="navbar-brand" href="#">Franklin</a>
            <button class="navbar-toggler" type="button" data-toggle="collapse" data-target="#navbarCollapse" aria-controls="navbarCollapse" aria-expanded="false" aria-label="Toggle navigation">
                <span class="navbar-toggler-icon"></span>
            </button>
            <div class="collapse navbar-collapse" id="navbarCollapse">
                <ul class="navbar-nav mr-auto">
                <li class="nav-item active">
                    <a class="nav-link" @click="componentToBeShown='Wallet'">Wallet</a>
                </li>
                <li class="nav-item">
                    <a class="nav-link" @click="componentToBeShown='History'">History</a>
                </li>
                <li class="nav-item">
                    <a class="nav-link disabled" href="#" tabindex="-1" aria-disabled="true">Disabled</a>
                </li>
                </ul>
            </div>
            </nav>
            <Alert v-bind:message="message"></Alert>
        </header>
        <Wallet 
            v-if="componentToBeShown=='Wallet'" 
            v-on:alert="displayAlert"
            v-bind:info="walletInfo"
            ></Wallet>
        <History v-if="componentToBeShown=='History'"></History>
    </b-container>
</template>

<script>
// TODO: remove this imports
import { ethers } from 'ethers'
import { Wallet as FranklinWallet, FranklinProvider } from 'franklin_lib'
import { WalletDecorator } from '../WalletDecorator'
// END-TODO

import History from './History.vue'
import Wallet from './Wallet.vue'
import Alert from '../components/Alert.vue'

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

const components = {
    History,
    Alert,
    Wallet
}

export default {
    name: 'Main',
    data: () => ({
        componentToBeShown: 'Wallet',
        walletInfo: null,
        message: null,
    }),
    async created() {
        // TODO: delete next block of code
        let franklinProvider = new FranklinProvider('http://localhost:3000', '0xc56E79CAA94C96DE01eF36560ac215cC7A4F0F47');
        // let signer = ethersProvider.getSigner();
        let provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
        window.signer = ethers.Wallet.fromMnemonic("fine music test violin matrix prize squirrel panther purchase material script deal", "m/44'/60'/0'/0/1").connect(provider);
        window.wallet = await FranklinWallet.fromEthWallet(signer, franklinProvider);
        window.walletDecorator = new WalletDecorator(window.wallet);

        this.updateAccountInfo();
    },
    methods: {
        displayAlert(msg) {
            this.message = msg;
        },
        async updateAccountInfo() {
            console.log('updated');
            await window.walletDecorator.updateState();
            let onchainBalances = window.walletDecorator.onchainBalancesAsRenderableList();
            let contractBalances = window.walletDecorator.contractBalancesAsRenderableList();
            let franklinBalances = window.walletDecorator.franklinBalancesAsRenderableList();
            let info = {
                onchainBalances,
                contractBalances,
                franklinBalances,
            };
            this.walletInfo = info;

            await sleep(2000);
            this.updateAccountInfo();
        }
    },
    components,
}
</script>
