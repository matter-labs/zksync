<template>
<div>
    <Navbar />
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <div v-if="loading">
            <img style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">
        </div>
        <div v-else>
            <h5>Supported Tokens</h5>
            <b-table 
                responsive 
                id="my-table" 
                hover 
                outlined 
                :items="tokens"
                :fields="['symbol', 'address', 'decimals']" 
                class="nowrap"
            >
                <template v-slot:cell(symbol)="data"><span v-html="data.item['symbol']" /></template>
                <template v-slot:cell(address)="data">
                    <a target="_blank" rel="noopener noreferrer" v-bind:href="`${urlForToken(data.item['address'])}`">
                        {{ data.item['address'] }} <i class="fas fa-external-link-alt"></i>
                    </a>
                </template>                
                <template v-slot:cell(decimals)="data"><span v-html="data.item['decimals']" /></template>
            </b-table>
        </div>
    </b-container>
</div>
</template>

<script>
import { clientPromise } from './Client';
import Navbar from './Navbar.vue';
const components = {
    Navbar,
};

export default {
    name: 'tokens',
    async created() {
        this.update();
    },
    data() { 
        return {
            tokens:             [],
            loading:            true,
            breadcrumbs:  [
                {
                    text: 'Home',
                    to: '/'
                },
                {
                    text: 'Tokens',
                    active: true
                },
            ],
        };
    },
    methods: {
        async update() {
            const client = await clientPromise;
            this.tokens = await client.loadTokens();
            this.tokens.sort((a,b) => a.symbol.localeCompare(b.symbol)).map(t => t.symbol);
            this.loading = false;
        },
        urlForToken(address) {
            const prefix = this.store.network === 'mainnet' ? '' : `${this.store.network}.`;
                return `https://${prefix}etherscan.io/token/${address}`;
        },
    },
    components,
};
</script>

<style>
</style>
