<template>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand :href="isMainPage ? '/' : '#'">
            <a href="https://zksync.io" target="_blank">
                <img class="navbar-hero-img" src="./assets/ZK_dark.svg">
            </a>
            
            <router-link v-if="!isMainPage" class="navbar-router-link" to="/">
                <b-badge variant="primary" class="hero-network-name">
                    {{store.capitalizedNetwork}}
                </b-badge>
            </router-link>
            <b-badge v-else variant="primary" class="hero-network-name">
                    {{store.capitalizedNetwork}}
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
                class="nowrap">
                Contract <span style="font-size: 0.9em"><i class="fas fa-external-link-alt"></i></span>
            </b-nav-item>
            <b-nav-item 
                class="nowrap">
                <router-link 
                    to='/tokens'
                    class="navbar-router-link"
                >Tokens</router-link>
            </b-nav-item>
            <b-nav-item 
                v-if="store.walletLink"
                v-bind:href="store.walletLink" 
                target="_blank" 
                rel="noopener noreferrer" 
                class="nowrap">
                Wallet <span style="font-size: 0.9em"><i class="fas fa-external-link-alt"></i></span>
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
    SearchField,
};

import store from './store';
import { blockchainExplorerAddress } from './constants';

export default {
    name: 'Navbar',
    components,
    data() {
        return {
            contractLink: `${blockchainExplorerAddress}/${store.contractAddress}`
        }
    },
    computed: {
        isMainPage() {
            return this.$router.currentRoute.path === '/';
        }
    },
    async created() {
        // console.log(this.$router.currentRoute);
        const client = await clientPromise;
        const { contractAddress } = await client.testnetConfig();
        this.store.contractAddress = contractAddress;
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
.navbar-router-link {
    color: inherit;
}
</style>
