<template>
<b-container id="login">
    <b-jumbotron bg-variant="light" border-variant="dark">
    <template slot="header">
        Matter Network Wallet <span style="font-size: 0.3em"><strong>ALPHA</strong></span>
    </template>
    <template slot="lead">
        <span v-if="isDev" class="local">API at {{apiServer}}</span>
        <span v-else></span>
    </template>
    <hr class="my-4">
    <b-btn v-if="ethereumSupported" variant="success" size="lg" @click="login">Login with Metamask</b-btn>
    <p v-else style="color: red">Ethereum support is not detected. Please use an Ethereum-compatible browser, e.g. install <a target="_blank" href="https://metamask.io">Metamask</a>.</p>
    </b-jumbotron>
</b-container>
</template>

<script>

import store from './store'
import ethUtil from 'ethjs-util'
import {newKey} from '../../franklin/src/transaction.js'
import {keccak256} from 'js-sha3'

export default {
    name: 'login',
    computed: {
        ethereumSupported: () => typeof window.web3 !== 'undefined',
    },
    methods: {
        async login() {
            try {
                let accounts = await eth.accounts()
                let account = accounts[0]
                this.acc = account
                if (!account) {
                    await ethereum.enable()
                    account = ethereum.selectedAddress
                }
                console.log('Logging in with', account)
                let sig = await eth.personal_sign(ethUtil.fromUtf8(new Buffer('Login Franklin v0.1')), account)
                store.account.address = account

                let hash = keccak256(sig)
                console.log('sig', sig)
                console.log('hash', hash)

                store.account.plasma.key = newKey(sig)
                console.log(store.account.plasma.key)

                this.$parent.$router.push('/wallet')
            } catch (e) {
                // TODO: replace with alert
                console.log('Login failed: ', e)
                this.err = e
            }
        }
    },
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