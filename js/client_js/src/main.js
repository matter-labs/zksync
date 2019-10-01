import Vue from "vue";
import App from "./App.vue";
import router from "./router";
import BootstrapVue from 'bootstrap-vue';
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

import { strCompareIgnoreCase } from './utils'
import config from "./env-config.js"

Vue.config.productionTip = false;

Vue.use(BootstrapVue);

const store = {
    pendingTransactionGenerators: [],
};

Vue.mixin({
	data: () => {
		return {
			isDev: process.env.NODE_ENV !== 'production',
			config,
		}
	},
	computed: {
        store() {
            return store;
        },
        ethereumAddress() {
            return window.walletDecorator.ethAddress;
        },
        franklinAddress() {
            return window.walletDecorator.address;
        },
        currentLocationNetworkName() {
            return this.config.ETH_NETWORK;
        },
        network() {
            return window.web3.version.network;
        },
        currentMetamaskNetworkName () {
            let net = ({
                '1': 'mainnet',
                '4': 'rinkeby',
                '9': 'localhost',
            })[this.network];
            if (net == undefined) return 'unknown';
            return net;
        },
        correctNetwork() {
            return strCompareIgnoreCase(this.currentMetamaskNetworkName, this.currentLocationNetworkName);
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
        ethereum.autoRefreshOnNetworkChange = true;
	}
}).$mount("#app");
