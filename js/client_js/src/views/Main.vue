<template>
    <div>
        <header>
            <nav class="navbar navbar-expand-md navbar-dark bg-info mb-4">
                <b-container>
                    <a class="navbar-brand" href="#">Matter Testnet</a>
                    <button 
                        class="navbar-toggler" type="button" data-toggle="collapse" 
                        data-target="#navbarCollapse" aria-controls="navbarCollapse" 
                        aria-expanded="false" aria-label="Toggle navigation">
                        <span class="navbar-toggler-icon"></span>
                    </button>
                    <div class="collapse navbar-collapse" id="navbarCollapse">
                        <ul class="navbar-nav mr-auto">
                            <li class="nav-item" :class="{active: componentToBeShown=='Wallet'}">
                                <a class="nav-link" @click="componentToBeShown='Wallet'">Wallet</a>
                            </li>
                            <li class="nav-item" :class="{active: componentToBeShown=='History'}">
                                <a class="nav-link" @click="componentToBeShown='History'">Transactions</a>
                            </li>
                        </ul>
                    </div>
                </b-container>
            </nav>
        </header>
        <b-container>
            <b-row class="w-100 m-0 p-0" style="position: relative">
                <b-col class="px-0">
                    <b style="color: red">Warning</b>: this app is for demo only. Database and smart contracts will be reset from time to time, 
                    with all coins lost!
                </b-col>
                <Alert class="w-100 m-0" style="position: absolute; top: -1.3em;" ref="alert"></Alert>
            </b-row>
            <b-row class="px-0 mt-4">
                <Wallet 
                    v-if="componentToBeShown=='Wallet'" 
                    v-on:alert="displayAlert"
                    :info="walletInfo"
                    ></Wallet>
                <History 
                    v-if="componentToBeShown=='History'"
                    :info="historyInfo"
                    ></History>
            </b-row>
        </b-container>
    </div>
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
        // let franklinProvider = new FranklinProvider('http://localhost:3000', '0xc56E79CAA94C96DE01eF36560ac215cC7A4F0F47');
        // let provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
        // window.signer = ethers.Wallet.fromMnemonic("fine music test violin matrix prize squirrel panther purchase material script deal", "m/44'/60'/0'/0/1").connect(provider);
        // window.wallet = await FranklinWallet.fromEthWallet(signer, franklinProvider);
        // window.walletDecorator = new WalletDecorator(window.wallet);

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
            let franklinBalancesWithInfo = window.walletDecorator.franklinBalancesAsRenderableListWithInfo();
            this.walletInfo = {
                onchainBalances,
                contractBalances,
                franklinBalances,
                franklinBalancesWithInfo,
            };

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

<style scoped>
.nav-item {
    cursor: pointer;
}
</style>
