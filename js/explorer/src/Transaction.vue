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
        <div v-else-if="transactionExists == false">
            <h5 class="mt-3">Can't find transaction <code> {{ tx_hash }} </code></h5>
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
import { clientPromise } from './Client';
import timeConstants from './timeConstants';

import SearchField from './SearchField.vue';

const components = {
    SearchField,
};

export default {
    name: 'transaction',
    data: () => ({
        txData: {},
        status: '',
        intervalHandle: null,
        loading: true,
        transactionExists: true,
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
            const client = await clientPromise;
            const tokens = await client.tokensPromise;

            const txData = await client.searchTx(this.tx_hash);
            if (txData == null) {
                this.transactionExists = false;
                return;
            }

            txData.tokenName = txData.token === -1 ? "" : tokens[txData.token].syncSymbol;
            txData.amount = txData.amount == "unknown amount" ? "" : txData.amount;
            
            const block = await client.getBlock(txData.block_number);
            txData.status = block.verified_at ? `Verified`
                           : block.committed_at ? `Committed`
                           : `unknown`;
            this.txData = txData;
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
                    text: 'Block ' + this.txData.block_number,
                    to: '/blocks/' + this.txData.block_number,
                },                
                {
                    text: 'Transaction ' + this.tx_hash,
                    active: true
                },
            ];
        },
        props() {
            if (Object.keys(this.txData).length == 0) 
                return [];

            const link_from 
                = this.txData.tx_type == 'Deposit' ? `${this.blockchainExplorerAddress}/${this.txData.from}`
                : `${this.routerBase}accounts/${this.txData.from}`;

            const link_to 
                = this.txData.tx_type == 'Withdraw' ? `${this.blockchainExplorerAddress}/${this.txData.to}`
                : this.txData.tx_type == 'ChangePubKeyOffchain' ? ''
                : `${this.routerBase}accounts/${this.txData.to}`;

            const onchain_from
                = this.txData.tx_type == 'Deposit' ? `<span class="onchain_icon">onchain</span>`
                : '';

            const onchain_to
                = this.txData.tx_type == 'Withdraw' ? `<span class="onchain_icon">onchain</span>`
                : '';

            const target_from
                = this.txData.tx_type == 'Deposit' ? `target="_blank" rel="noopener noreferrer"`
                : '';

            const target_to
                = this.txData.tx_type == 'Withdraw' ? `target="_blank" rel="noopener noreferrer"`
                : '';

            const rows = [
                { name: 'Tx hash',        value: `<code>${this.tx_hash}</code>`},
                { name: "Type",           value: `<b>${this.txData.tx_type}</b>`   },
                { name: "Status",         value: `<b>${this.txData.status}</b>` },
                { name: "From",           value: `<code><a ${target_from} href="${link_from}">${this.txData.from} ${onchain_from}</a></code>`      },
                { name: "To",             value: `<code><a ${target_to} href="${link_to}">${this.txData.to} ${onchain_to}</a></code>`      },
                { name: "Amount",         value: `<b>${this.txData.tokenName}</b> ${readableEther(this.txData.amount)}`    },
            ];

            if (this.txData.fee) 
                rows.push(
                    { name: "fee",            value: this.txData.fee       });

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
