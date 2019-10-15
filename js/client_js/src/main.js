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
    verboseShowerId: 0,
};

Vue.mixin({
	data: () => {
		return {
			isDev: process.env.NODE_ENV !== 'production',
            config
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
        baseUrl() {
            return this.apiServer + '/api/v0.1'
        },
        currentLocationNetworkName() {
            return this.config.ETH_NETWORK;
        },
    },
});

new Vue({
	router,
	render: h => h(App),
	async created() {
        ethereum.autoRefreshOnNetworkChange = false;
        const checkNetwork = () => {
            window.web3.version.getNetwork((err, currentNetwork) => {
                let net = ({
                    '1': 'mainnet',
                    '4': 'rinkeby',
                    '9': 'localhost',
                })[currentNetwork]
                || 'unknown';
                                
                let correct = this.config.ETH_NETWORK.toLowerCase() == net.toLowerCase();
                if (correct == false) {
                    if (router.currentRoute.path !== '/login') {
                        router.push('/login');
                    }
                }
                if (router.currentRoute.path === '/login') {
                    if (window.web3 == false) {
                        document.getElementById("change_network_alert").style.display = "none";
                        document.getElementById("login_button").style.display = "none";
                    } else if (correct) {
                        document.getElementById("change_network_alert").style.display = "none";
                        document.getElementById("login_button").style.display = "inline-block";
                    } else {
                        document.getElementById("change_network_alert").style.display = "inline-block";
                        document.getElementById("login_button").style.display = "none";
                    }
                }
            });
        };

        checkNetwork(); // the first time
        setInterval(checkNetwork, 1000);
	},
}).$mount("#app");
