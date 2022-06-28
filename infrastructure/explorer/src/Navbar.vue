<template>
    <b-navbar toggleable="md" type="dark" variant="light">
        <b-container>
            <b-navbar-brand>
                <a href="/" target="_blank">
                    <img class="navbar-hero-img" src="./assets/rif_logo.svg" />
                </a>
                <b-badge variant="primary" class="hero-network-name pointer" v-on:click.prevent="goToHome">
                    {{ store.capitalizedNetwork }}
                </b-badge>
            </b-navbar-brand>
            <b-navbar-toggle target="nav-collapse"></b-navbar-toggle>
            <b-collapse id="nav-collapse" is-nav>
                <b-navbar-nav>
                    <!-- <b-nav-item href="/client/" target="_blank" rel="noopener noreferrer">zkSync Wallet</b-nav-item> -->
                    <b-nav-item
                        v-if="store.contractAddress"
                        v-bind:href="`${contractLink}`"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="nowrap"
                    >
                        Contract
                        <span style="font-size: 0.9em"><i class="fas fa-external-link-alt"></i></span>
                    </b-nav-item>
                    <b-nav-item class="nowrap" v-on:click.prevent="goToTokens"> Tokens </b-nav-item>
                    <b-nav-item
                        v-if="store.walletLink"
                        v-bind:href="store.walletLink"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="nowrap"
                    >
                        Wallet
                        <span style="font-size: 0.9em"><i class="fas fa-external-link-alt"></i></span>
                    </b-nav-item>
                </b-navbar-nav>
                <b-navbar-nav class="ml-auto">
                    <b-nav-form>
                        <SearchField :searchFieldInMenu="true" />
                    </b-nav-form>
                </b-navbar-nav>
            </b-collapse>
        </b-container>
    </b-navbar>
</template>

<script>
import SearchField from './SearchField.vue';
import { clientPromise } from './Client';
const components = {
    SearchField
};

import store from './store';
import { blockchainExplorerAddress } from './constants';

export default {
    name: 'Navbar',
    components,
    computed: {
        contractLink() {
            return `${blockchainExplorerAddress}/${store.contractAddress}`;
        }
    },
    methods: {
        goToTokens() {
            if (this.$router.currentRoute.path === '/tokens') {
                return;
            }

            this.$router.push('/tokens');
        },
        goToHome() {
            if (this.$router.currentRoute.path === '/') {
                return;
            }

            this.$router.push('/');
        }
    },
    async created() {
        const client = await clientPromise;
        const { contractAddress } = await client.testnetConfig();
        store.contractAddress = contractAddress;
    }
};
</script>

<style scoped>
.navbar-hero-img {
    margin-bottom: 1px;
    width: 6em;
}
.hero-network-name {
    margin-left: 0.6em;
    color: #eee;
    font-size: 0.8em;
}
.pointer {
    cursor: pointer;
}

.navbar-dark .navbar-nav .nav-link {
    color: #4d4d4d;
}
.navbar-dark .navbar-nav .nav-link:hover, 
.navbar-dark .navbar-nav .nav-link:active, 
.navbar-dark .navbar-nav .nav-link:focus {
    color: #000;
}
</style>
