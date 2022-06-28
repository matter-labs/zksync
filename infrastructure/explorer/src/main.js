import Vue from 'vue';
import BootstrapVue from 'bootstrap-vue';

import './style.css';

import Clipboard from 'v-clipboard';

import store from './store';

import Router from 'vue-router';
import App from './App.vue';
import Home from './Home.vue';
import Block from './Block.vue';
import Transaction from './Transaction.vue';
import Account from './Account.vue';
import Tokens from './Tokens.vue';

import VueTimers from 'vue-timers';

const ethers = require('ethers');

Vue.use(VueTimers);
Vue.use(Router);
Vue.use(BootstrapVue);
Vue.use(Clipboard);

const routes = [
    { path: '/', component: Home },
    { path: '/blocks/:blockNumber', component: Block },
    { path: '/transactions/:id', component: Transaction },
    { path: '/accounts/:address', component: Account, props: true },
    { path: '/tokens', component: Tokens }
];

function getRouterBase() {
    return process.env.NODE_ENV === 'production' ? '/explorer/' : '/';
}

const router = new Router({
    routes, // short for `routes: routes`
    mode: 'history',
    base: getRouterBase()
});

Vue.mixin({
    data: () => ({
        store,
        routerBase: getRouterBase()
    })
});

window.app = new Vue({
    el: '#app',
    router,
    created() {
        document.title = `RIF Aggregation ${store.capitalizedNetwork} Explorer — trustless scalable payments`;
    },
    render: (h) => h(App)
});

// debug utils

window.store = store;
window.ethers = ethers;
window.p = {
    // promise printer for debugging in console
    set p(promise) {
        promise.then((r) => console.log(r));
    }
};
