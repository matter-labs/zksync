import Vue from 'vue'
import BootstrapVue from "bootstrap-vue"
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

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
    render: h => h(App)
})
