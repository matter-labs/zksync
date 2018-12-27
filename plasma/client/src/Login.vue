<template>
    <b-jumbotron bg-variant="light" border-variant="dark">
    <template slot="header">
        Plasma Wallet
    </template>
    <template slot="lead">
        Plasma on SNARKs has arrived. ethereumSupport = {{ ethereumSupport }}
    </template>
    <hr class="my-4">
    <b-btn variant="success" size="lg" @click="login">Login with Metamask</b-btn>
    </b-jumbotron>
</template>

<script>

import { ethers } from 'ethers'

import Wallet from './Wallet.vue'

export default {
    name: 'login',
    data () {
        return {
            web3: new ethers.providers.Web3Provider(web3.currentProvider),
            accounts: null,
        }
    },
    computed: {
        ethereumSupport: () => typeof window.ethereum !== 'undefined'
    },
    methods: {
        async login() {
            try {
                this.accounts = await ethereum.enable()
                this.$parent.$router.push('/wallet')
            } catch (e) {
                console.log('login failed: ', e)
            }
        }
    },
}
</script>
