import Vue from 'vue';
import BootstrapVue from "bootstrap-vue";

import Clipboard from 'v-clipboard';

import store from './store';

import Router from 'vue-router';
import App from './App.vue';
import Home from './Home.vue';
import Block from './Block.vue';
import Transaction from './Transaction.vue';
import Account from './Account.vue';

import config from './env-config';
import VueTimers from 'vue-timers';

const ethers = require('ethers');

Vue.use(VueTimers);
Vue.use(Router);
Vue.use(BootstrapVue);
Vue.use(Clipboard);

const routes = [
    { path: '/',                    component: Home  },
    { path: '/blocks/:blockNumber', component: Block },
    { path: '/transactions/:id',    component: Transaction },
    { path: '/accounts/:address',   component: Account, props: true },
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
    methods: {
        formatFranklin(value) {
            return ethers.utils.formatEther(ethers.utils.bigNumberify(value).mul(1000000000000));
        },
    },
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
});

window.app = new Vue({
    el: '#app',
    router,
    async created() {
        this.store.config = config;
        let regex = /(?:api-)*(\w*)(?:\..*)*/;
        this.store.network = this.store.config.ETH_NETWORK;
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
