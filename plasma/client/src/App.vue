<template>
    <div id="app">
        <b-container>
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
        </b-container>
        <ul v-for="account in accounts">
            <li>{{account}}</li>
        </ul>
    </div>
</template>

<script>

import { ethers } from 'ethers'

export default {
    name: 'app',
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
