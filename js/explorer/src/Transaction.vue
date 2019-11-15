<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Matter Testnet</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <h5 class="mt-3">Transaction data</h5>
        <img 
            src="./assets/loading.gif" 
            width="100" 
            height="100"
            v-if="loading">
        <b-card v-else no-body class="table-margin-hack">
            <b-table responsive thead-class="hidden_header" :items="props">
                <template v-slot:cell(value)="data"><span v-html="data.item['value']" /></template>
            </b-table>
        </b-card>
        <br>
    </b-container>
</div>
</template>

<script>

import store from './store';
import { readableEther } from './utils';
import client from './client';
import timeConstants from './timeConstants';

export default {
    name: 'transaction',
    data: () => ({
        tx_data: {},
        status: '',
        intervalHandle: null,
        loading: true,
    }),
    async created() {
        this.update();
        this.intervalHandle = setInterval(() => {
            this.update();
        }, timeConstants.transactionUpdate);
    },
    methods: {
        async update() {
            this.loading = true;
            let tx_data = await this.fraProvider.getTransactionByHash(this.tx_hash);
            tx_data.tokenName = (await this.tokensPromise)[tx_data.token].symbol;
            let block = await client.getBlock(tx_data.block_number);
            tx_data.status = block.verified_at ? `Verified`
                           : block.committed_at ? `Committed`
                           : `unknown`;
            this.tx_data = tx_data;
            this.loading = false;
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

            let rows = [
                { name: 'Tx hash',        value: `<code>${this.tx_hash}</code>`},
                { name: "Type",           value: `<b>${this.tx_data.tx_type}</b>`   },
                { name: "Status",         value: `<b>${this.tx_data.status}</b>` },
                { name: "From",           value: `<code><a target="_blanc" href="${link_from}">${this.tx_data.from}</a></code>`      },
                { name: "To",             value: `<code><a target="_blanc" href="${link_to}">${this.tx_data.to}</a></code>`      },
                { name: "Amount",         value: `<b>${this.tx_data.tokenName}</b> ${readableEther(this.tx_data.amount)}`    },
            ];

            if (this.tx_data.fee) 
                rows.push(
                { name: "fee",            value: this.tx_data.fee       });

            return rows;
        },
    },
};
</script>

<style>
.table-margin-hack table, 
.table-margin-hack .table-responsive {
    margin: 0 !important;
}

</style>
