import Vue from 'vue'
import BootstrapVue from "bootstrap-vue"
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import store from './store'
import { ethers } from 'ethers'

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
    routes // short for `routes: routes`
})

window.app = new Vue({
    el: '#app',
    router,
    data: () => ({
        store
    }),
    async created() {

        // read store.account from local storage?

        if (typeof window.ethereum !== 'undefined') {
            this.store.web3 = new ethers.providers.Web3Provider(web3.currentProvider)
            let accounts = await ethereum.enable()
            if (store.account !== accounts[0]) {
                // switching accounts
                store.account = accounts[0]
                this.$router.push('/login')
            }
        }
    },
    render: h => h(App)
})

window.p = {
    // promise printer for debugging in console
    set p(promise) {
        promise.then(r => console.log(r) )
    },
}