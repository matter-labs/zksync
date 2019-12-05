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
    Alert
};

import Eth from 'ethjs'
import { ethers } from 'ethers'
import { Wallet, FranklinProvider } from 'franklin_lib'
import { WalletDecorator } from '../WalletDecorator'

export default {
    name: 'login',
    computed: {
        ethereumSupported: () => typeof window.web3 !== 'undefined',
    },
    methods: {
        async login() {
            try {
                await window.ethereum.enable();
                window.eth = new Eth(window.ethereum);
                window.ethersProvider = new ethers.providers.Web3Provider(window.ethereum);
                let franklinProvider = new FranklinProvider(this.config.API_SERVER, this.config.CONTRACT_ADDR);
                let signer = window.ethersProvider.getSigner();
                window.wallet = await Wallet.fromEthWallet(signer, franklinProvider);
                window.walletDecorator = await WalletDecorator.new(window.wallet);

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
    components
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
