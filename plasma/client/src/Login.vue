<template>
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
</template>

<script>

import store from './store'
import ethUtil from 'ethjs-util'

export default {
    name: 'login',
    computed: {
        ethereumSupported: () => typeof window.ethereum !== 'undefined'
    },
    methods: {
        async login() {
            try {
                let account = (await ethereum.enable()) [0]
                console.log(account)
                let sig = await eth.personal_sign(ethUtil.fromUtf8(new Buffer('Login to Plasma Wallet')), account)
                console.log(sig)
                store.account = account
                this.$parent.$router.push('/wallet')
            } catch (e) {
                console.log('login failed: ', e)
            }
        }
    },
}
</script>
