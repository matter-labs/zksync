import Vue from "vue";
import Router from "vue-router";

import Login from './views/Login.vue'
import Wallet from './views/Wallet.vue'

Vue.use(Router);

export default new Router({
  routes: [
    { path: '/login', component: Login },
    { path: '/wallet', component: Wallet },
    { path: '*', redirect: '/login' },
  ],
  mode:   'history',
});
