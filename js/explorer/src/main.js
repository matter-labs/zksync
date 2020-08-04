import Vue from 'vue';
import BootstrapVue from "bootstrap-vue";

import "./style.css";

import Clipboard from 'v-clipboard';

import store from './store';

import Router from 'vue-router';
import App from './App.vue';
import Home from './Home.vue';
import Block from './Block.vue';
import Transaction from './Transaction.vue';
import Account from './Account.vue';
import Tokens from './Tokens.vue';

import config from './env-config';
import VueTimers from 'vue-timers';
import { capitalize, sleep } from './utils';

const ethers = require('ethers');

Vue.use(VueTimers);
Vue.use(Router);
Vue.use(BootstrapVue);
Vue.use(Clipboard);

const routes = [
    { path: '/',                    component: Home },
    { path: '/blocks/:blockNumber', component: Block },
    { path: '/transactions/:id',    component: Transaction },
    { path: '/accounts/:address',   component: Account, props: true },
    { path: '/tokens',              component: Tokens },
];

function getRouterBase() {
    return process.env.NODE_ENV === 'production' ? '/explorer/' : '/';
}

const router = new Router({
    routes, // short for `routes: routes`
    mode: 'history',
    base: getRouterBase(),
});

Vue.mixin({
    data: () => ({
        store,
        routerBase: getRouterBase(),
    }),
    computed: {
        blockchainExplorerTx() {
            return this.store.network === 'localhost' ? 'http://localhost:8000'
                 : this.store.network === 'mainnet'   ? `https://etherscan.io/tx`
                 : `https://${this.store.network}.etherscan.io/tx`;
        },
        blockchainExplorerAddress() {
            return this.store.network === 'localhost' ? 'http://localhost:8000'
                 : this.store.network === 'mainnet'   ? `https://etherscan.io/address`
                 : `https://${this.store.network}.etherscan.io/address`;
        },
    },
    methods: {
        blockchainExplorerToken(token, account) {
            if (this.store.network === 'localhost') return `http://localhost:8000/${account}`;
            const prefix = this.store.network === 'mainnet' ? '' : `${this.store.network}.`;
            const tokenAddress = window.syncProvider.tokenSet.resolveTokenAddress(token);
            
            if (tokenAddress != '0x0000000000000000000000000000000000000000') {
                return `https://${prefix}etherscan.io/token/${tokenAddress}?a=${account}`;
            } else {
                return `https://${prefix}etherscan.io/address/${account}`;
            }
        }
    }
});

window.app = new Vue({
    el: '#app',
    router,
    async created() {
        this.store.config = config;
        let regex = /(?:api-)*(\w*)(?:\..*)*/;
        this.store.network = this.store.config.ETH_NETWORK;
        this.store.capitalizedNetwork = capitalize(this.store.network);
        const walletLinkPrefix = this.store.network == 'mainnet' ? 'wallet' : this.store.network;
        this.store.walletLink = `https://${walletLinkPrefix}.zksync.io`;

        (async () => {
            while (!this.store.capitalizedNetwork) await sleep(100);
            document.title = `zkSync ${this.store.capitalizedNetwork} Explorer — trustless scalable payments`;    
        })();
    },
    render: h => h(App)
});

// debug utils

window.store = store;
window.ethers = ethers;
window.p = {
    // promise printer for debugging in console
    set p(promise) {
        promise.then(r => console.log(r) );
    },
};
