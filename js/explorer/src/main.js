import Vue from 'vue'
import BootstrapVue from "bootstrap-vue"
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import store from './store'

import Router from 'vue-router'
import App from './App.vue'
import Home from './Home.vue'
import Block from './Block.vue'
import Transaction from './Transaction.vue'

import axios from 'axios'
import url from 'url'
import config from './env-config'
import VueTimers from 'vue-timers'

Vue.use(VueTimers)
Vue.use(Router)
Vue.use(BootstrapVue)

const routes = [
    { path: '/', component: Home },
    { path: '/blocks/:blockNumber', component: Block },
    { path: '/transactions/:id', component: Transaction },
]

const router = new Router({
    routes, // short for `routes: routes`
    mode: 'history',
    base: '/explorer'
})

Vue.mixin({
    data() {
        return {
            store
        }
    },
    computed: {
        etherscan() {
            if (this.store.network === 'localhost') return 'http://localhost:4000'
            return 'https://' + (this.store.network === 'mainnet' ? '' : `${this.store.network}.`) + 'etherscan.io'
        },
    },
})

window.app = new Vue({
    el: '#app',
    router,
    async created() {
        this.store.config = config
        let regex = /(?:api-)*(\w*)(?:\..*)*/
        this.store.network = 
            regex.exec(url.parse(this.store.config.API_SERVER).host)[1]
    },
    render: h => h(App)
})

// debug utils

window.store = store
window.p = {
    // promise printer for debugging in console
    set p(promise) {
        promise.then(r => console.log(r) )
    },
}