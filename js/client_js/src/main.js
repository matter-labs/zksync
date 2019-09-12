import Vue from "vue";
import App from "./App.vue";
import router from "./router";
import BootstrapVue from 'bootstrap-vue';
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import {ethers} from 'ethers';
import Eth from 'ethjs'

Vue.config.productionTip = false;

Vue.use(BootstrapVue);

new Vue({
  router,
  render: h => h(App),
  async created() {
    window.ethereum.enable();
    window.eth = new Eth(window.web3.currentProvider);
    window.ethersProvider = new ethers.providers.Web3Provider(window.web3.currentProvider);
  }
}).$mount("#app");
