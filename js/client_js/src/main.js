import Vue from "vue";
import App from "./views/App.vue";
import router from "./router";
import store from './store'
import BootstrapVue from 'bootstrap-vue';
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import {ethers} from 'ethers';
import Eth from 'ethjs'

Vue.config.productionTip = false;

Vue.use(BootstrapVue);

Vue.mixin({
  computed: {
    store: () => store,
    isDev: () => process.env.NODE_ENV === 'development',
    apiServer() { return "this.store.config.API_SERVER" },
  },
});


new Vue({
  router,
  render: h => h(App),
  async created() {
    window.ethereum.enable();
    window.eth = new Eth(window.web3.currentProvider);
    window.ethersProvider = new ethers.providers.Web3Provider(window.web3.currentProvider);
  }
}).$mount("#app");
