import Vue from 'vue'
import BootstrapVue from "bootstrap-vue"
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import store from './store'
import Eth from 'ethjs'
import {ethers} from 'ethers'

window.ethers = ethers;
window.Eth = Eth

import Router from 'vue-router'
import App from './App.vue'
import Login from './Login.vue'
import Wallet from './Wallet.vue'

Vue.use(Router)
Vue.use(BootstrapVue)

const routes = [
    { path: '/login', component: Login },
    { path: '/wallet', component: Wallet },
    { path: '*', redirect: '/login' },
]

const router = new Router({
    routes, // short for `routes: routes`
    base: '/client'
})

Vue.mixin({
    computed: {
        isDev: () => process.env.NODE_ENV === 'development',
        apiServer: () => process.env.API_SERVER,
    },
})

import ABI from './contract'

window.app = new Vue({
    el: '#app',
    router,
    data: () => ({
        store
    }),
    async created() {
        // read store.account from local storage?
        if (typeof window.web3 !== 'undefined') {
            window.eth = new Eth(web3.currentProvider)
            window.ethersProvider = new ethers.providers.Web3Provider(web3.currentProvider)

            // const provider = new ethers.providers.JsonRpcProvider('http://localhost:8545')
            // window.c = new ethers.Contract(
            //     '0x2a1780C1EDbE60f6667818bc4b3402004A9e9559',  // proxy
            //     //'0x2AeBa6973B6D0104e927D945321cE8ddFDE7c5a6', // depositor
            //     //'0xbcB1385a441464345040F33E59e166fCC2720F04', // tx
            //     ABI, provider)
        }
        if (!store.account.address) {
            this.$router.push('/login')
        }
    },
    render: h => h(App)
})

// debug utils

window.BN = require('bn.js')
window.Buffer = require('buffer/').Buffer
window.store = store
window.p = {
    // promise printer for debugging in console
    set p(promise) {
        promise.then(r => console.log(r) )
    },
}