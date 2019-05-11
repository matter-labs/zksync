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

Vue.use(Router)
Vue.use(BootstrapVue)

const routes = [
    { path: '/', component: Home },
    { path: '/blocks/:blockNumber', component: Block },
    { path: '/transactions/:id', component: Transaction },
    //{ path: '*', redirect: '/login' },
]

const router = new Router({
    routes, // short for `routes: routes`
    mode: 'history'
})

Vue.mixin({
    computed: {
        apiServer: () => process.env.API_SERVER,
    },
})

window.app = new Vue({
    el: '#app',
    router,
    data: () => ({
        store
    }),
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