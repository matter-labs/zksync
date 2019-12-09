<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand href="/">Matter Network</b-navbar-brand>
        <b-navbar-nav class="ml-auto">
            <b-nav-form>
                <SearchField :searchFieldInMenu="true" />
            </b-nav-form>
        </b-navbar-nav>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <div v-if="loading">
            <h5 class="mt-3">Transaction data</h5>
            <img 
            src="./assets/loading.gif" 
            width="100" 
            height="100">
        </div>
        <div v-else>
            <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
            <h5 class="mt-3">Transaction data</h5>
            <b-card no-body class="table-margin-hack">
                <b-table responsive thead-class="hidden_header" :items="props">
                    <template v-slot:cell(value)="data"><span v-html="data.item['value']" /></template>
                </b-table>
            </b-card>
            <br>
        </div>
    </b-container>
</div>
</template>

<script>

import store from './store';
import { readableEther } from './utils';
import client from './client';
import timeConstants from './timeConstants';

import SearchField from './SearchField.vue';

const components = {
    SearchField,
};

export default {
    name: 'transaction',
    data: () => ({
        tx_data: {},
        status: '',
        intervalHandle: null,
        loading: true,
    }),
    async created() {
        await this.update();
        this.loading = false;
        this.intervalHandle = setInterval(() => {
            this.update();
        }, timeConstants.transactionUpdate);
    },
    destroyed() {
        clearInterval(this.intervalHandle);
    },
    methods: {
        async update() {
            let tx_data = await this.fraProvider.getTransactionByHash(this.tx_hash);
            tx_data.tokenName = (await this.tokensPromise)[tx_data.token].symbol;
            let block = await client.getBlock(tx_data.block_number);
            tx_data.status = block.verified_at ? `Verified`
                           : block.committed_at ? `Committed`
                           : `unknown`;
            this.tx_data = tx_data;
        },
    },
    computed: {
        tx_hash() {
            return this.$route.params.id;
        },
        breadcrumbs() {
            return [
                {
                    text: 'All blocks',
                    to: '/'
                },
                {
                    text: 'Block ' + this.tx_data.block_number,
                    to: '/blocks/' + this.tx_data.block_number,
                },                
                {
                    text: 'Transaction ' + this.tx_hash,
                    active: true
                },
            ];
        },
        props() {
            if (Object.keys(this.tx_data).length == 0) 
                return [];

            let link_from 
                = this.tx_data.tx_type == 'Deposit' ? `${this.blockchain_explorer_address}/${this.tx_data.from}`
                : `/accounts/${this.tx_data.from}`;

            let link_to 
                = this.tx_data.tx_type == 'Withdraw' ? `${this.blockchain_explorer_address}/${this.tx_data.to}`
                : `/accounts/${this.tx_data.to}`;

            let onchain_from
                = this.tx_data.tx_type == 'Deposit' ? `<span class="onchain_icon">onchain</span>`
                : '';

            // <i class="fas fa-external-link-alt" />
            let onchain_to
                = this.tx_data.tx_type == 'Withdraw' ? `<span class="onchain_icon">onchain</span>`
                : '';

            let target_from
                = this.tx_data.tx_type == 'Deposit' ? `target="_blank" rel="noopener noreferrer"`
                : '';

            // <i class="fas fa-external-link-alt" />
            let target_to
                = this.tx_data.tx_type == 'Withdraw' ? `target="_blank" rel="noopener noreferrer"`
                : '';

            let rows = [
                { name: 'Tx hash',        value: `<code>${this.tx_hash}</code>`},
                { name: "Type",           value: `<b>${this.tx_data.tx_type}</b>`   },
                { name: "Status",         value: `<b>${this.tx_data.status}</b>` },
                { name: "From",           value: `<code><a ${target_from} href="${link_from}">${this.tx_data.from} ${onchain_from}</a></code>`      },
                { name: "To",             value: `<code><a ${target_to} href="${link_to}">${this.tx_data.to} ${onchain_to}</a></code>`      },
                { name: "Amount",         value: `<b>${this.tx_data.tokenName}</b> ${readableEther(this.tx_data.amount)}`    },
            ];

            if (this.tx_data.fee) 
                rows.push(
                { name: "fee",            value: this.tx_data.fee       });

            return rows;
        },
    },
    components,
};
</script>

<style>
.table-margin-hack table, 
.table-margin-hack .table-responsive {
    margin: 0 !important;
}

.onchain_icon {
    display: inline-block;
    line-height: 1.5em;
    font-weight: bold;
    background: #17a2b8;
    border-radius: 5px;
    padding: 0 .2em;
    color: white;
}
</style>
