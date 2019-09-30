import Vue from "vue";
import App from "./App.vue";
import router from "./router";
import BootstrapVue from 'bootstrap-vue';
import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap-vue/dist/bootstrap-vue.css"

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
            return window.walletDecorator.ethAddress;
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
                '1': 'mainnet',
                '4': 'rinkeby',
                '9': 'localhost',
            })[this.network];
        },
        currentMetamaskNetwork() {
            return this.config.ETH_NETWORK;
        },
        correctNetwork() {
            return this.currentMetamaskNetworkName == this.currentMetamaskNetwork;
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
        ethereum.autoRefreshOnNetworkChange = false;
	}
}).$mount("#app");
