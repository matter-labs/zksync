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
    mode: 'history',
    base: '/explorer'
})

Vue.mixin({
    // computed: {
    //     store:  () => store,
    // },
    data() {
        return {
            store
        }
    },
})

window.app = new Vue({
    el: '#app',
    router,
    async created() {
        if (process.env.NODE_ENV !== 'development') {
            let r = await axios({
                method:     'get',
                url:        'config.json',
            })
            if (r.status === 200) {
                this.store.config = r.data
                console.log(store.config)
            }
        } else {
            this.store.config = {}
        }
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