import Vue from "vue";
import Router from "vue-router";

import Login from './views/Login.vue'
import Main from './views/Main.vue'

Vue.use(Router);

export default new Router({
  routes: [
    { path: '/login', component: Login },
    { path: '/main', component: Main },
    { path: '*', redirect: '/login' },
  ],
  mode:   'history',
});
