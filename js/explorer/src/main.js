import Vue from 'vue';
import BootstrapVue from "bootstrap-vue";
import "bootstrap/dist/css/bootstrap.min.css";
import "bootstrap-vue/dist/bootstrap-vue.css";

import Clipboard from 'v-clipboard';

import store from './store';

import Router from 'vue-router';
import App from './App.vue';
import Home from './Home.vue';
import Block from './Block.vue';
import Transaction from './Transaction.vue';
import Account from './Account.vue';

import axios from 'axios';
import url from 'url';
import config from './env-config';
import VueTimers from 'vue-timers';
import { WalletDecorator } from './WalletDecorator';
import { FranklinProvider } from 'franklin_lib';

const ethers = require('ethers');

Vue.use(VueTimers);
Vue.use(Router);
Vue.use(BootstrapVue);
Vue.use(Clipboard);

const routes = [
    { path: '/', component: Home },
    { path: '/blocks/:blockNumber', component: Block },
    { path: '/transactions/:id', component: Transaction },
    { path: '/accounts/:address', component: Account }
];

const router = new Router({
    routes, // short for `routes: routes`
    mode: 'history',
    base:   process.env.NODE_ENV === 'production' ? '/explorer/' : '/',
});

let fraProvider = new FranklinProvider(config.API_SERVER, config.CONTRACT_ADDR);
let tokensPromise = fraProvider.getTokens();

Vue.mixin({
    data: () => ({
        store,
        fraProvider,
        tokensPromise,
    }),
    methods: {
        formatFranklin(value) {
            return ethers.utils.formatEther(ethers.utils.bigNumberify(value).mul(1000000000000));
        },
        // parseFranklin(value) {
        //     return ethers.utils.parseEther(value).div(1)
        // },
    },
    computed: {
        blockchain_explorer_tx() {
            return this.store.network === 'localhost' ? 'http://localhost:8000'
                 : this.store.network === 'mainnet'   ? `https://etherscan.io/tx`
                 : `https://${this.store.network}.etherscan.io/tx`;
        },
        blockchain_explorer_address() {
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
