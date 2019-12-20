<template>
<b-container id="login">
    <Alert class="w-100 mb-1" ref="alertLogin"></Alert>
    <b-jumbotron bg-variant="light" border-variant="dark">
        <template slot="header">
            ZK Sync Wallet <span style="font-size: 0.3em"><strong>ALPHA</strong></span>
        </template>
        <template slot="lead">
            <span v-if="isDev" class="local">API at {{ config.API_SERVER }}</span>
        </template>
        <hr class="my-4">
        <p v-if="!ethereumSupported" style="color: red">Ethereum support is not detected. Please use an Ethereum-compatible browser, e.g. install <a target="_blank" href="https://metamask.io">Metamask</a>.</p>
        <h3 id="change_network_alert" style="color: red; display: none">
            Please switch to <strong>{{ currentLocationNetworkName }}</strong> network in Metamask to try this demo.
        </h3>
        <b-btn id="login_button" style="display: none" variant="success" size="lg" @click="login">Login with Metamask</b-btn>
    </b-jumbotron>
</b-container>
</template>

<script>
import Alert from '../components/Alert.vue'

const components = {
    Alert,
};

const ethers = require('ethers');
const zksync = require('zksync');

import { WalletDecorator } from '../WalletDecorator'

export default {
    name: 'login',
    computed: {
        ethereumSupported: () => typeof window.web3 !== 'undefined',
    },
    methods: {
        async login() {
            try {
                const net = this.currentLocationNetworkName == 'localhost' 
                    ? 'localhost'
                    : 'testnet';
                const syncProvider = await zksync.getDefaultProvider(net);
                
                const tokensList = await syncProvider.getTokens()
                window.tokensList = Object.values(tokensList)
                    .map(token => ({
                        ...token,
                        symbol: token.symbol || `${token.id.toString().padStart(3, '0')}`,
                    }))
                    .sort((a, b) => a.id - b.id);

                await window.ethereum.enable();
                const ethersProvider = new ethers.providers.Web3Provider(window.ethereum);
                const ethProxy = new zksync.ETHProxy(ethersProvider, syncProvider.contractAddress);
                
                const signer = ethersProvider.getSigner();
                const syncWallet = await zksync.Wallet.fromEthSigner(signer, syncProvider, ethProxy);

                window.ethProvider = ethersProvider;
                window.ethSigner = signer;
                window.syncWallet = syncWallet;
                window.syncProvider = syncProvider;
                window.ethProxy = ethProxy;

                window.walletDecorator = await WalletDecorator.new();

                this.$parent.$router.push('/main')
            } catch (e) {
                this.$refs.alertLogin.display({
                    message: `Login failed with ${e.message}`,
                    variant: 'info',
                    countdown: 10,
                });
            }
        }
    },
    components,
}
</script>

<style>
#login {
    font-family: 'Avenir', Helvetica, Arial, sans-serif;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-align: center;
    color: #2c3e50;
    margin-top: 60px;
}

.local {
    color: yellow;
    background: navy;
    padding: 0.5em;
    margin: 1em;
}

h1, h2 {
    font-weight: normal;
}
</style>
