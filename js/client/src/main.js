import Vue from 'vue'
import BootstrapVue from "bootstrap-vue"
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import store from './store'
import Eth from 'ethjs'
import {ethers} from 'ethers'
import axios from 'axios'
import url from 'url'
import config from './env-config'

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
    routes,
    mode:   'history',
    base:   '/client',
})

Vue.mixin({
    computed: {
        store: () => store,
        isDev: () => process.env.NODE_ENV === 'development',
        apiServer() { return this.store.config.API_SERVER },
    },
})

import ABI from './contract'

window.app = new Vue({
    el: '#app',
    router,
    data: () => ({
        storeMain: store
    }),
    async created() {
        this.store.config = config

        let regex = /(?:api-)*(\w*)(?:\..*)*/
        this.store.network = 
            regex.exec(url.parse(this.store.config.API_SERVER).host)[1]

        // read store.account from local storage?
        if (typeof window.web3 !== 'undefined') {
            window.eth = new Eth(web3.currentProvider)
            window.ethersProvider = new ethers.providers.Web3Provider(web3.currentProvider)
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