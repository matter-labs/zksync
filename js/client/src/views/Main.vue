<template>
    <div>
        <header>
            <b-navbar toggleable="md" type="dark" variant="info" class="mb-4">
                <b-container>
                    <a href="/explorer/"><b-navbar-brand>ZK Sync Devnet</b-navbar-brand></a>
                    <b-navbar-toggle target="nav-collapse"></b-navbar-toggle>
                    <b-collapse id="nav-collapse" is-nav>
                        <b-navbar-nav target>
                            <b-nav-item :class="{active: componentToBeShown=='Wallet'}"  @click="componentToBeShown='Wallet'">Wallet</b-nav-item>
                            <b-nav-item :class="{active: componentToBeShown=='History'}"  @click="componentToBeShown='History'">Transactions</b-nav-item>
                        </b-navbar-nav>
                    </b-collapse>
                </b-container>
            </b-navbar>
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
                <b-row class="px-3 py-0 my-0" v-for="verboseOp in store.pendingTransactionGenerators" :key="verboseOp.id" :id="verboseOp.id">
                    <AlertWithProgressBar :verboseOp="verboseOp"></AlertWithProgressBar>
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
        window.walletDecorator.on(
            "receiptCommittedOrVerified", 
            () => {
                // console.log('receiptCommittedOrVerified');
                this.updateAccountInfo();
            }
        );

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

                let onchainBalances          =       window.walletDecorator.onchainBalancesAsRenderableList();
                let franklinBalances         =       window.walletDecorator.franklinBalancesAsRenderableList();
                let franklinBalancesWithInfo =       window.walletDecorator.franklinBalancesAsRenderableListWithInfo();
                let pendingOps               = await window.walletDecorator.pendingOperationsAsRenderableList();

                this.walletInfo = {
                    onchainBalances,
                    franklinBalances,
                    franklinBalancesWithInfo,
                    pendingOps,
                };

            } catch (e) {
                console.log('updateaccountinfo error:', e);
                let message = e.message;
                let franklinServerReachable = await isReachable(this.config.API_SERVER);
                if (franklinServerReachable == false) {
                    message = "ZK Sync server unavailable, check your internet connection.";
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
