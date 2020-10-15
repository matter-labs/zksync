<template>
<div>
    <Navbar />
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <h5 class="mt-3 mb-2">Account data</h5>
        <b-card no-body class="table-margin-hack">
            <b-table responsive thead-class="displaynone" class="nowrap" :items="accountDataProps">
                <template v-slot:cell(value)="data">
                    <CopyableAddress
                        class="bigger-text"
                        :address="address" 
                        :linkHtml="data.item['value']"
                        :tooltipRight="true"
                    />
                </template>
            </b-table>
        </b-card>
        <h5 class="mt-3 mb-2">Account balances</h5>
        <img 
            src="./assets/loading.gif" 
            width="100" 
            height="100"
            v-if="loading">
        <div v-else-if="balances.length == 0">
            No balances yet.
        </div>
        <b-card v-else no-body class="table-margin-hack table-width-hack">
            <b-table responsive thead-class="displaynone" class="nowrap" :items="balancesProps">
                <template v-slot:cell(value)="data"><span v-html="data.item.value" /></template>
            </b-table>
        </b-card>
        <h5 class="mt-3 mb-2">Account transactions</h5>
        <img 
            src="./assets/loading.gif" 
            width="100" 
            height="100"
            v-if="loading">
        <div v-else-if="transactions.length == 0">
            No transactions yet.
        </div>
        <div v-else>
            <b-card no-body class="table-margin-hack">
                <b-table
                    responsive 
                    class="nowrap"
                    :items="transactionProps" 
                    :fields="transactionFields">
                    <template v-slot:cell(TxHash) ="data">
                        <i v-if="!data.item['success']" class="fas fa-times brown" />
                        <span v-html="data.item['TxHash']" />
                    </template>
                    <template v-slot:cell(Type)   ="data"><span v-html="data.item['Type']"   /></template>
                    <template v-slot:cell(Block)  ="data"><span v-html="data.item['Block']"  /></template>
                    <template v-slot:cell(Value)  ="data"><span v-html="data.item['Value']"  /></template>
                    <template v-slot:cell(Amount) ="data"><span v-html="data.item['Amount']" /></template>
                    <template v-slot:cell(Age)    ="data"><span v-html="data.item['Age']"    /></template>
                    <template v-slot:cell(From)   ="data"><span v-html="data.item['From']"   /></template>
                    <template v-slot:cell(To)     ="data"><span v-html="data.item['To']"     /></template>
                    <template v-slot:cell(Fee)    ="data"><span v-html="data.item['Fee']"    /></template>
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

<script>

import store from './store';
import timeConstants from './timeConstants';
import { clientPromise } from './Client';
import { shortenHash, formatDate } from './utils';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
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
    watch: {
        async currentPage() {
            await this.update();
        }
    },
    async created() {
        this.client = await clientPromise;

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
            const balances = await this.client.getCommitedBalances(this.address);
            this.balances = balances
                .map(bal => ({ name: bal.tokenSymbol, value: bal.balance }));

            const offset = (this.currentPage - 1) * this.rowsPerPage;
            const limit = this.rowsPerPage;

            // maybe load the requested page
            if (this.pagesOfTransactions[this.currentPage] == undefined)
                this.pagesOfTransactions[this.currentPage] 
                    = await this.client.transactionsList(this.address, offset, limit);

            let nextPageLoaded = false;
            let numNextPageTransactions;
            
            // maybe load the next page
            if (this.pagesOfTransactions[this.currentPage + 1] == undefined) {
                let txs = await this.client.transactionsList(this.address, offset + limit, limit);
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
                { name: 'Address',          value: `<a>${this.address}</a> ` },
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
                    if (tx.hash.startsWith('sync-tx:')) {
                        tx.hash = tx.hash.slice('sync-tx:'.length);
                    }

                    if (tx.type == 'Withdraw') {
                        tx.type = 'Withdrawal';
                    }

                    let TxHash = `
                        <a href="${this.routerBase}transactions/${tx.hash}">
                            ${shortenHash(tx.hash, 'unknown! hash')}
                        </a>`;

                    const link_from = tx.type == 'Deposit' 
                        ? `${this.blockchainExplorerAddress}/${tx.from}`
                        : `${this.routerBase}accounts/${tx.from}`;

                    const link_to = tx.type == 'Withdrawal' 
                        ? `${this.blockchainExplorerAddress}/${tx.to}`
                        : `${this.routerBase}accounts/${tx.to}`;

                    const target_from = tx.type == 'Deposit' 
                        ? `target="_blank" rel="noopener noreferrer"`
                        : '';

                    const target_to = tx.type == 'Withdrawal' 
                        ? `target="_blank" rel="noopener noreferrer"`
                        : '';

                    const onchain_from = tx.type == 'Deposit' 
                        ? '<i class="fas fa-external-link-alt"></i> '
                        : '';

                    const onchain_to = tx.type == 'Withdrawal' 
                        ? '<i class="fas fa-external-link-alt"></i> '
                        : '';

                    const From = `
                        <a href="${link_from}" ${target_from}>
                            ${shortenHash(tx.from, 'unknown! from')}
                            ${onchain_from}
                        </a>`;

                    const To = `
                        <a href="${link_to}" ${target_to}>
                            ${
                                tx.type == "ChangePubKey" 
                                    ? ''
                                    : shortenHash(tx.to, 'unknown! to')
                            }

                            ${ tx.type == "ChangePubKey" ? '' : onchain_to }
                        </a>`;

                    const Type = `${tx.type}`;
                    const Amount 
                        = tx.type == "ChangePubKey" ? ''
                        : `${tx.token} <span>${tx.amount}</span>`;
                    const CreatedAt = formatDate(tx.created_at);

                    return {
                        TxHash,
                        Type,
                        Amount,
                        From, 
                        To,
                        CreatedAt,
                        success: tx.success,
                        fromAddr: tx.from,
                        toAddr: tx.to,
                        hash: tx.hash,
                    };
                });
        },
        transactionFields() {
            return this.transactionProps && this.transactionProps.length
                 ? Object.keys(this.transactionProps[0])
                    .filter(k => ! ['hash', 'fromAddr', 'toAddr', 'success'].includes(k))
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
.normalize-text {
    font-size: 1.0em;
}
.bigger-text {
    font-size: 1.05em;
}
.brown {
    color: #AA935D;
}
</style>
