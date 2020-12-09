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
                    <template v-slot:cell(TxHash) = "data">
                        <i v-if="!data.item['success']" class="fas fa-times brown" />
                        <Entry
                            :value="data.item['TxHash'].value"
                        />
                    </template>
                    <template v-slot:cell(Type) = "data">
                        <Entry
                            :value="data.item['Type'].value"
                        />
                    </template>
                    <template v-slot:cell(Block) = "data">
                        <Entry
                            :value="data.item['Block'].value"
                        />
                    </template>
                    <template v-slot:cell(Amount) = "data">
                       <Entry
                            :value="data.item['Amount'].value"
                        />
                    </template>
                    <template v-slot:cell(From) = "data">
                        <Entry
                            :value="data.item['From'].value"
                        />
                    </template>
                    <template v-slot:cell(To) = "data">
                        <Entry
                            :value="data.item['To'].value"
                        />
                    </template>
                    <template v-slot:cell(CreatedAt) = "data">
                        <Entry
                            :value="data.item['CreatedAt'].value"
                        />
                    </template>

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

import timeConstants from './timeConstants';
import { clientPromise } from './Client';
import { shortenHash, formatDate, makeEntry } from './utils';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';

import { blockchainExplorerAddress } from './constants';
import Entry from './links/Entry.vue';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
    Entry
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
        nextAddress: ''
    }),
    watch: {
        async currentPage() {
            await this.update();
        },
        nextAddress() {
            this.loading = true;
            this.totalRows = 0;
            this.pagesOfTransactions = {};

            this.update();
        }
    },
    beforeRouteUpdate(to, from, next) {
        if(to.params.address) {
            this.currentPage = 1;
            this.nextAddress = to.params.address;
        }

        next();
    },
    beforeRouteEnter (to, from, next) {
        if(to.params.address) {
            next(vm => vm.nextAddress = to.params.address);
        }
        next();
    },
    async created() {
        this.client = await clientPromise;

        if(!this.nextAddress) {
            this.nextAddress = this.address;
        }

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
            // Used to tackle races when 
            // the page updates, but this async function
            // tries to update the page, according to the previous address
            const addressAtBeginning = this.nextAddress;

            const balances = await this.client.getCommitedBalances(addressAtBeginning);
            this.balances = balances
                .map(bal => ({ name: bal.tokenSymbol, value: bal.balance }));

            const offset = (this.currentPage - 1) * this.rowsPerPage;
            const limit = this.rowsPerPage;

            // maybe load the requested page
            if (this.pagesOfTransactions[this.currentPage] == undefined)
                this.pagesOfTransactions[this.currentPage] 
                    = await this.client.transactionsList(addressAtBeginning, offset, limit);

            let nextPageLoaded = false;
            let numNextPageTransactions;
            
            // maybe load the next page
            if (this.pagesOfTransactions[this.currentPage + 1] == undefined) {
                let txs = await this.client.transactionsList(addressAtBeginning, offset + limit, limit);
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

            if(this.nextAddress === addressAtBeginning) {
                this.loading = false;
            }
        },
        loadNewTransactions() {
            this.totalRows = 0;
            this.pagesOfTransactions = {};
            this.load();
        },
        getHashEntry(tx) {
            if (tx.hash.startsWith('sync-tx:')) {
                tx.hash = tx.hash.slice('sync-tx:'.length);
            }

            return makeEntry('TxHash')
                .localLink(`${this.routerBase}transactions/${tx.hash}`)
                .innerHTML(`${shortenHash(tx.hash, 'unknown! hash')}`);
        },
        getLinkFromEntry(tx) {
            const entry = makeEntry('From');

            if (tx.type == 'Deposit') {
                entry.outterLink(`${blockchainExplorerAddress}/${tx.from}`);
            } else {
                entry.localLink(`${this.routerBase}accounts/${tx.from}`);
            }

            return entry.innerHTML(`${shortenHash(tx.from, 'unknown! from')}`);
        },
        getLinkToEntry(tx) {
            const entry = makeEntry('To');
            
            if(tx.type == "ChangePubKey" ) {
                return entry;
            }

            if(tx.type == 'Withdrawal') {
                entry
                    .outterLink(`${blockchainExplorerAddress}/${tx.to}`);
            } else {
                entry
                    .localLink(`${this.routerBase}accounts/${tx.to}`);
            }
            
            return entry.innerHTML(`${shortenHash(tx.to, 'unknown! to')}`);
        },
        getTxAmountEntry() {
            
        }
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
                    if (tx.type == 'Withdraw') {
                        tx.type = 'Withdrawal';
                    }

                    const TxHash = this.getHashEntry(tx);
                    const From = this.getLinkFromEntry(tx);

                    const To = this.getLinkToEntry(tx);

                    const Type = makeEntry('Type').innerHTML(tx.type);
                    const Amount = makeEntry('Amount')
                        .innerHTML(`${tx.token} <span>${tx.amount}</span>`);
                    const CreatedAt = makeEntry('CreatedAt')
                        .innerHTML(formatDate(tx.created_at));
                    
                    if(tx.type === 'ChangePubKey') {
                        // There is no amount for ChangePubKey
                        Amount.innerHTML('');
                    }

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
