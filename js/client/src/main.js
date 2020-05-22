import Vue from "vue";
import App from "./App.vue";
import router from "./router";
import BootstrapVue from 'bootstrap-vue';
import config from "./env-config.js";
import { sleep } from './utils';

Vue.config.productionTip = false;

Vue.use(BootstrapVue);

const store = {
    pendingTransactionGenerators: [],
    verboseOperationId: 0,
};

Vue.mixin({
	data: () => ({
        isDev: process.env.NODE_ENV !== 'production',
        config,
    }),
	computed: {
        store() {
            return store;
        },
        ethereumAddress() {
            return window.walletDecorator.ethAddress;
        },
        franklinAddress() {
            return window.syncWallet.address();
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
        window.ethereum.autoRefreshOnNetworkChange = false;
        const checkNetwork = async () => {
            let net = ({
                '1': 'mainnet',
                '4': 'rinkeby',
                '3': 'ropsten',
                '9': 'localhost',
            })[window.ethereum.networkVersion]
            || 'unknown';

            let networkCorrect = this.currentLocationNetworkName.toLowerCase() == net.toLowerCase();
            if (!networkCorrect) {
                if (router.currentRoute.path !== '/login') {
                    router.push('/login');
                }
            }
            if (router.currentRoute.path === '/login') {
                while (!document.getElementById("change_network_alert")) {
                    await sleep(1000);
                }

                if (!window.ethereum) {
                    document.getElementById("change_network_alert").style.display = "none";
                    document.getElementById("login_button").style.display = "none";
                } else if (networkCorrect) {
                    document.getElementById("change_network_alert").style.display = "none";
                    document.getElementById("login_button").style.display = "inline-block";
                } else {
                    document.getElementById("change_network_alert").style.display = "inline-block";
                    document.getElementById("login_button").style.display = "none";
                }
            }
        };
        window.ethereum.on('chainIdChanged', checkNetwork);
        window.ethereum.on('accountsChanged', accounts => {
            if (router.currentRoute.path !== '/login') {
                router.push('/login');
            }
        });

        checkNetwork(); // the first time

        // this isn't needed if window.ethereum.on handler works
        // but it doesn't work on Metamask Plugins Beta.
        setInterval(checkNetwork, 1000);
	},
}).$mount("#app");
