<template>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand href="/">zkSync Network</b-navbar-brand>
        <b-navbar-toggle target="nav-collapse"></b-navbar-toggle>
        <b-collapse id="nav-collapse" is-nav>
        <b-navbar-nav>
            <!-- <b-nav-item href="/client/" target="_blank" rel="noopener noreferrer">zkSync Wallet</b-nav-item> -->
            <b-nav-item 
                v-bind:href="`${blockchainExplorerAddress}/${store.contractAddress}`" 
                target="_blank" 
                rel="noopener noreferrer" 
                class="nowrap">
                Contract <span style="font-size: 0.9em"><i class="fas fa-external-link-alt"></i></span>
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
const components = {
    SearchField,
};

export default {
    name: 'Navbar',
    components,
}
</script>
