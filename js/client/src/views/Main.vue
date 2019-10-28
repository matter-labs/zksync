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
            <div style="min-height: 1.5em;">
                <b-row class="px-3 py-0 my-0" v-for="shower in store.pendingTransactionGenerators" :key="shower.id" :id="shower.id">
                    <AlertWithProgressBar :shower="shower"></AlertWithProgressBar>
                </b-row>
            </div>
            <b-row class="px-0 mt-0">
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
import ClipboardJS from 'clipboard'
import isReachable from 'is-reachable'

import Wallet from '../components/Wallet.vue'
import History from '../components/History.vue'
import Alert from '../components/Alert.vue'
import AlertWithProgressBar from '../components/AlertWithProgressBar.vue'

import { sleep } from '../utils.js'
import timeConstants from '../timeConstants'

const components = {
    History,
    Alert,
    Wallet,
    AlertWithProgressBar,
};

export default {
    name: 'Main',
    data: () => ({
        componentToBeShown: 'Wallet',
        walletInfo: null,
        historyInfo: null,
        message: null,
    }),
    watch: {
        async componentToBeShown() {
            await this.updateAccountInfo()
        },
    },
    async created() {
        const timeOut = async () => {
            await this.updateAccountInfo();
            await sleep(timeConstants.updateInfo);
            timeOut();
        };
        timeOut();

        new ClipboardJS('.copyable');
    },
    methods: {
        displayAlert(options) {
            this.$refs.alert.display(options);
        },
        async updateAccountInfo() {
            try {
                await window.walletDecorator.updateState();

                let onchainBalances = window.walletDecorator.onchainBalancesAsRenderableList();
                let contractBalances = window.walletDecorator.contractBalancesAsRenderableList();
                let franklinBalances = window.walletDecorator.franklinBalancesAsRenderableList();
                let franklinBalancesWithInfo = window.walletDecorator.franklinBalancesAsRenderableListWithInfo();
                let pendingOps = await window.walletDecorator.pendingOperationsAsRenderableList();

                this.walletInfo = {
                    onchainBalances,
                    contractBalances,
                    franklinBalances,
                    franklinBalancesWithInfo,
                    pendingOps,
                };

            } catch (e) {
                console.log('updateaccountinfo error:', e);
                let message = e.message;
                let franklinServerReachable = await isReachable(this.config.API_SERVER);
                if (franklinServerReachable == false) {
                    message = "Franklin server unavailable, check your internet connection.";
                }
                
                this.displayAlert({
                    message: message,
                    variant: 'danger',
                    countdown: 10,
                })
            }
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
