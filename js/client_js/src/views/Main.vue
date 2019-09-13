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
                    <li class="nav-item" v-bind:class="{active: componentToBeShown=='Wallet'}">
                        <a class="nav-link" @click="componentToBeShown='Wallet'">Wallet</a>
                    </li>
                    <li class="nav-item" v-bind:class="{active: componentToBeShown=='History'}">
                        <a class="nav-link" @click="componentToBeShown='History'">History</a>
                    </li>
                </ul>
            </div>
            </nav>
            <Alert ref="alert"></Alert>
        </header>
        <Wallet 
            v-if="componentToBeShown=='Wallet'" 
            v-on:alert="displayAlert"
            v-bind:info="walletInfo"
            ></Wallet>
        <History 
            v-if="componentToBeShown=='History'"
            v-bind:info="historyInfo"
            ></History>
    </b-container>
</template>

<script>
// TODO: remove this imports
import { ethers } from 'ethers'
import { Wallet as FranklinWallet, FranklinProvider } from 'franklin_lib'
import { WalletDecorator } from '../WalletDecorator'
// END-TODO

import Wallet from '../components/Wallet.vue'
import History from '../components/History.vue'
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
        historyInfo: null,
        message: null,
    }),
    watch: {
        componentToBeShown: async function() {
            await this.updateAccountInfo()
        }
    },
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
        displayAlert(kwargs) {
            this.$refs.alert.display(kwargs)
        },
        async updateAccountInfo() {
            await window.walletDecorator.updateState();
            let onchainBalances = window.walletDecorator.onchainBalancesAsRenderableList();
            let contractBalances = window.walletDecorator.contractBalancesAsRenderableList();
            let franklinBalances = window.walletDecorator.franklinBalancesAsRenderableList();
            let walletInfo = {
                onchainBalances,
                contractBalances,
                franklinBalances,
            };
            this.walletInfo = walletInfo;

            this.historyInfo = {
                transactions: window.walletDecorator.transactionsAsNeeded()
            };

            await sleep(3000);
            this.updateAccountInfo();
        }
    },
    components,
}
</script>
