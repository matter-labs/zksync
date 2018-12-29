<template>
<b-container id="login">
    <b-jumbotron bg-variant="light" border-variant="dark">
    <template slot="header">
        Plasma Wallet
    </template>
    <template slot="lead">
        Plasma on SNARKs has arrived
    </template>
    <hr class="my-4">
    <b-btn v-if="ethereumSupported" variant="success" size="lg" @click="login">Login with Metamask</b-btn>
    <p v-else style="color: red">Ethereum support is not detected. Please use an Ethereum-compatible browser, e.g. install <a href="https://metamask.io">Metamask</a>.</p>
    </b-jumbotron>
</b-container>
</template>

<script>

import store from './store'
import ethUtil from 'ethjs-util'
import {newKey} from '../../contracts/lib/transaction.js'
import {keccak256} from 'js-sha3'

export default {
    name: 'login',
    computed: {
        ethereumSupported: () => typeof window.ethereum !== 'undefined'
    },
    methods: {
        async login() {
            try {
                console.log('login')
                let account = ethereum.selectedAddress
                if (!account) {
                    console.log('enable')
                    await ethereum.enable()
                    account = ethereum.selectedAddress
                }
                console.log('with', account)
                let sig = await eth.personal_sign(ethUtil.fromUtf8(new Buffer('Login to Plasma Wallet')), account)
                console.log(sig)
                store.account.address = account

                let hash = keccak256(sig)
                store.account.plasma.key = newKey(hash)
                console.log(store.account.plasma.key)

                this.$parent.$router.push('/wallet')
            } catch (e) {
                console.log('login failed: ', e)
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

h1, h2 {
    font-weight: normal;
}

</style>