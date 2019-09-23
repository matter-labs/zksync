import Vue from "vue";
import App from "./App.vue";
import router from "./router";
import BootstrapVue from 'bootstrap-vue';
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import {ethers} from 'ethers';
import Eth from 'ethjs'

import config from "./env-config.js"

Vue.config.productionTip = false;

Vue.use(BootstrapVue);

Vue.mixin({
	data: () => {
		return {
			isDev: true, // TODO
			config,
		}
	},
	computed: {
        ethereumAddress() {
            return window.walletDecorator.wallet.ethWallet.address;
        },
        franklinAddress() {
            return window.walletDecorator.address;
        },
        currentLocationNetworkName() {
            return window.location.host.split(/[^\w]/)[0];
        },
        network() {
            return window.web3.version.network;
        },
        currentMetamaskNetworkName () {
            return ({
                '1': 'Mainnet',
                '4': 'Rinkeby',
                '9': 'localhost',
            })[this.network];
        },
        currentMetamaskNetwork() {
            return window.location.hostname.split('.')[0];
        },
        correctNetwork() {
            return (this.network === '9' && window.location.hostname.startsWith('localhost')) ||
                (this.network === '1' && window.location.hostname.startsWith('mainnet')) ||
                (this.network === '4' && window.location.hostname.startsWith('rinkeby'));
        },
        baseUrl() {
            return this.apiServer + '/api/v0.1'
        },
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
