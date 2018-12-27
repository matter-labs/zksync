<template>
    <b-jumbotron bg-variant="light" border-variant="dark">
    <template slot="header">
        Plasma Wallet
    </template>
    <hr class="my-4">
        <ul v-for="account in accounts">
            <li>{{account}}</li>
        </ul>
    </b-jumbotron>
</template>

<script>

import { ethers } from 'ethers'

export default {
    name: 'wallet',
    data () {
        return {
            web3Provider: new ethers.providers.Web3Provider(web3.currentProvider),
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
            } catch (e) {
                console.log('login failed: ', e)
            }
        }
    },
}
</script>

<style>
#app {
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
