import Vue from "vue";
import Router from "vue-router";

import Login from './views/Login.vue'
import Main from './views/Main.vue'

Vue.use(Router);


const router = new Router({
  routes: [
    { path: '/login', component: Login, meta: {title: 'zkSync Wallet'} },
    { path: '/main',  component: Main,  meta: {title: 'zkSync Wallet'} },
    { path: '*', redirect: '/login' },
  ],
  mode:   'history',
  base:   process.env.NODE_ENV === 'production' ? '/client/' : '/',
});

router.beforeEach((to, from, next) => {
    if (to.fullPath === '/main') {
        if (!window.walletDecorator) {
            next('/login');
        }
    }
    if (to.fullPath === '/login') {
        if (window.walletDecorator) {
            next('/main');
        }
    }
    document.title = to.meta.title;
    next();
});

export default router;
