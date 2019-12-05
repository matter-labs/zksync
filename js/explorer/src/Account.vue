<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
        <b-container>
            <b-navbar-brand href="/">ZK Sync Network</b-navbar-brand>
            <b-navbar-nav class="ml-auto">
                <b-nav-form>
                    <SearchField :searchFieldInMenu="true" />
                </b-nav-form>
            </b-navbar-nav>
        </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-breadcrumb class="" :items="breadcrumbs"></b-breadcrumb>
        <h5 class="mt-3 mb-2">Account data</h5>
        <b-card no-body class="table-margin-hack">
            <b-table responsive thead-class="hidden_header" class="my-0 py-0" :items="accountDataProps">
                <template v-slot:cell(value)="data"><span v-html="data.item.value" /></template>
            </b-table>
        </b-card>
        <h5 class="mt-3 mb-2">Account balances</h5>
        <img 
            src="./assets/loading.gif" 
            width="100" 
            height="100"
            v-if="loading">
        <div v-else-if="balances.length == 0" style="font-size: 2em">
            No balances yet.
        </div>
        <b-card v-else no-body class="table-margin-hack table-width-hack">
            <b-table responsive thead-class="hidden_header" :items="balancesProps">
                <template v-slot:cell(value)="data"><span v-html="data.item.value" /></template>
            </b-table>
        </b-card>
        <h5 class="mt-3 mb-2">Account transactions</h5>
        <img 
            src="./assets/loading.gif" 
            width="100" 
            height="100"
            v-if="loading">
        <div v-else-if="transactions.length == 0" style="font-size: 2em">
            No transactions yet.
        </div>
        <div v-else>
            <b-card no-body class="table-margin-hack">
                <b-table
                    responsive 
                    class="clickable"
                    :items="transactionProps" 
                    :fields="transactionFields" 
                    @row-clicked="onRowClicked">
                    <template v-slot:cell(Type)   ="data"><span v-html="data.item['Type']"    /></template>
                    <template v-slot:cell(TxnHash)="data"><span v-html="data.item['TxnHash']" /></template>
                    <template v-slot:cell(Block)  ="data"><span v-html="data.item['Block']"   /></template>
                    <template v-slot:cell(Value)  ="data"><span v-html="data.item['Value']"   /></template>
                    <template v-slot:cell(Amount) ="data"><span v-html="data.item['Amount']"  /></template>
                    <template v-slot:cell(Age)    ="data"><span v-html="data.item['Age']"     /></template>
                    <template v-slot:cell(From)   ="data"><span v-html="data.item['From']"    /></template>
                    <template v-slot:cell(To)     ="data"><span v-html="data.item['To']"      /></template>
                    <template v-slot:cell(Fee)    ="data"><span v-html="data.item['Fee']"     /></template>
                </b-table>
            </b-card>
            <b-pagination 
                v-if="this.pagesOfTransactions[2] && this.pagesOfTransactions[2].length"
                class="mt-2 mb-2"
                v-model="currentPage" 
                :per-page="rowsPerPage" 
                :total-rows="totalRows"
                hide-goto-end-buttons
            ></b-pagination>
        </div>
    </b-container>
</div>
</template>

<style>
.hidden_header {
  display: none;
}
</style>

<script>

import store from './store';
import timeConstants from './timeConstants';
import { WalletDecorator } from './WalletDecorator';
import { readableEther } from './utils';

import SearchField from './SearchField.vue';

const components = {
    SearchField,
};

export default {
    name: 'Account',
    data: () => ({
        balances: [],
        transactions: [],
        pagesOfTransactions: {},

        currentPage: 1,
        rowsPerPage: 10,
        totalRows: 0,

        loading: true,

        intervalHandle: null,
        client: null,
    }),
    async created() {
        this.client = new WalletDecorator(this.address, this.fraProvider);

        this.update();  
        this.intervalHandle = setInterval(async () => {
            if (this.currentPage == 1) {
                this.pagesOfTransactions = {};
                await this.update();
            }
        }, timeConstants.accountUpdate);
    },
    destroyed() {
        clearInterval(this.intervalHandle);
    },
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.hash);
        },
        async update() {
            let balances = await this.client.getCommitedBalances();
            this.balances = balances
                .map(bal => ({ name: bal.tokenName, value: bal.balance }));

            let offset = (this.currentPage - 1) * this.rowsPerPage;
            let limit = this.rowsPerPage;

            // maybe load the requested page
            if (this.pagesOfTransactions[this.currentPage] == undefined)
                this.pagesOfTransactions[this.currentPage] 
                    = await this.client.getTransactions(offset, limit);
            

            let nextPageLoaded = false;
            let numNextPageTransactions;
            
            // maybe load the next page
            if (this.pagesOfTransactions[this.currentPage + 1] == undefined) {
                let txs = await this.client.getTransactions(offset + limit, limit);
                numNextPageTransactions = txs.length;
                nextPageLoaded = true;

                // Once we assign txs to pagesOfTransactions,
                // it gets wrapped in vue watchers and stuff.
                // 
                // Sometimes this.pagesOfTransactions[this.currentPage + 1].length
                // is > limit, which I can only explain by vue's wrapping.
                // Hopefully, this will fix it.
                this.pagesOfTransactions[this.currentPage + 1] = txs;
            }

            if (nextPageLoaded) {
                // we now know if we can add a new page button
                this.totalRows = offset + limit + numNextPageTransactions;
            }

            // display the page
            this.transactions = this.pagesOfTransactions[this.currentPage];

            this.loading = false;
        },
        loadNewTransactions() {
            this.totalRows = 0;
            this.pagesOfTransactions = {};
            this.load();
        },
    },
    computed: {
        address() {
            return this.$route.params.address;
        },
        accountDataProps() {
            return [
                { name: 'Address',          value: `<code>${this.address}</code>`},
            ];
        },
        balancesProps() {
            return this.balances;
        },
        breadcrumbs() {
            return [
                {
                    text: 'Home',
                    to: '/'
                },
                {
                    text: 'Account '+this.address,
                    active: true
                },
            ];
        },
        transactionProps() {
            return this.transactions
                .map(tx => {
                    let TxnHash = `<code>
                        <a href="/transactions/${tx.data.hash}" target="_blank" rel="noopener noreferrer">
                            ${tx.data.hash.slice(0, 8)}..${tx.data.hash.slice(-8)}
                        </a>
                    </code>`;                    

                    let link_from
                        = tx.data.type == 'Deposit' ? `${this.blockchain_explorer_address}/${tx.data.from}`
                        : `/accounts/${tx.data.from}`;

                    let link_to
                        = tx.data.type == 'Withdraw' ? `${this.blockchain_explorer_address}/${tx.data.to}`
                        : `/accounts/${tx.data.to}`;

                    let target_from
                        = tx.data.type == 'Deposit' ? `target="_blank" rel="noopener noreferrer"`
                        : '';

                    let target_to
                        = tx.data.type == 'Withdraw' ? `target="_blank" rel="noopener noreferrer"`
                        : '';

                    let From = `<code>
                        <a href="${link_from}" ${target_from}>
                            ${tx.data.from.slice(0, 8)}..${tx.data.from.slice(-8)}
                        </a>
                    </code>`;

                    let To = `<code>
                        <a href="${link_to}" ${target_to}>
                            ${tx.data.to.slice(0, 8)}..${tx.data.to.slice(-8)}
                        </a>
                    </code>`;

                    let Type = `<b>${tx.data.type}</b>`;
                    let Amount = `<b>${tx.data.token}</b> <span>${tx.data.amount}</span>`;

                    return {
                        Type,
                        TxnHash,
                        Amount,
                        From, 
                        To,

                        hash: tx.data.hash,
                    };
                });
        },
        transactionFields() {
            return this.transactionProps && this.transactionProps.length
                 ? Object.keys(this.transactionProps[0]).filter(k => k != 'hash')
                 : [];
        }
    },
    components,
};
</script>

<style>
.table-margin-hack table, 
.table-margin-hack .table-responsive {
    margin: 0 !important;
}

.table-width-hack td:first-child {
    width: 10em;
}
</style>
