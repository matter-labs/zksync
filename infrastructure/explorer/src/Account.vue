<template>
    <div>
        <br />
        <b-container>
            <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
            <h5 class="mt-3 mb-2">Account data</h5>
            <b-card no-body class="table-margin-hack">
                <b-table responsive thead-class="displaynone" class="nowrap" :items="accountDataProps">
                    <template v-slot:cell(value)="data">
                        <Entry class="bigger-text" v-if="data.item.name == 'Address'" :value="data.item.value" />
                        <Entry v-else-if="data.item.name == 'Account Id'" :value="data.item.value" />
                        <Entry v-else-if="data.item.name == 'Verified nonce'" :value="data.item.value" />
                        <Entry v-else-if="data.item.name == 'Committed nonce'" :value="data.item.value" />
                    </template>
                </b-table>
            </b-card>
            <h5 class="mt-3 mb-2">Account balances</h5>
            <img src="./assets/loading.gif" width="100" height="100" v-if="loading" />
            <div v-else-if="balances.length == 0">No balances yet.</div>
            <b-card v-else no-body class="table-margin-hack table-width-hack">
                <b-table responsive thead-class="displaynone" class="nowrap" :items="balancesProps">
                    <template v-slot:cell(value)="data"><span v-html="data.item.value" /></template>
                </b-table>
            </b-card>
            <div class="alternativeWithdrawMsg" v-if="eligibleForForcedExit">
                Funds from this account can be moved to L1 using the
                <outter-link :to="alternativeWithdrawAddressLink" innerHTML="Alternative Withdraw"></outter-link>
            </div>
            <h5 class="mt-3 mb-2">Account transactions</h5>
            <img src="./assets/loading.gif" width="100" height="100" v-if="loading" />
            <div v-else-if="transactions.length == 0">No transactions yet.</div>
            <div v-else>
                <b-card no-body class="table-margin-hack">
                    <b-table responsive class="nowrap" :items="transactionProps" :fields="transactionFields">
                        <template v-slot:cell(TxHash)="data">
                            <i v-if="!data.item['success']" class="fas fa-times brown mr-1" />
                            <Entry :value="data.item['TxHash'].value" />
                        </template>
                        <template v-slot:cell(Type)="data">
                            <Entry :value="data.item['Type'].value" />
                        </template>
                        <template v-slot:cell(Block)="data">
                            <Entry :value="data.item['Block'].value" />
                        </template>
                        <template v-slot:cell(Amount)="data">
                            <Entry :value="data.item['Amount'].value" />
                        </template>
                        <template v-slot:cell(From)="data">
                            <Entry :value="data.item['From'].value" />
                        </template>
                        <template v-slot:cell(To)="data">
                            <Entry :value="data.item['To'].value" />
                        </template>
                        <template v-slot:cell(CreatedAt)="data">
                            <Entry :value="data.item['CreatedAt'].value" />
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
import OutterLink from './links/OutterLink';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import { accountStateToBalances, makeEntry, isEligibleForForcedExit } from './utils';
import Navbar from './Navbar.vue';
import store from './store';

import Entry from './links/Entry.vue';

import { getTxEntries } from './accountTxEntries';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
    Entry,
    OutterLink
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
        eligibleForForcedExit: false,

        intervalHandle: null,
        client: null,
        nextAddress: '',
        accountId: 'loading...',
        verifiedNonce: 'loading...',
        committedNonce: 'loading...'
    }),
    watch: {
        async currentPage() {
            await this.update();
        },
        async nextAddress() {
            this.loading = true;
            this.totalRows = 0;
            this.eligibleForForcedExit = false;
            this.pagesOfTransactions = {};

            this.client = await clientPromise;

            this.update();
        }
    },
    beforeRouteUpdate(to, from, next) {
        if (to.params.address) {
            this.currentPage = 1;
            this.nextAddress = to.params.address;
        }

        next();
    },
    beforeRouteEnter(to, from, next) {
        if (to.params.address) {
            next((vm) => (vm.nextAddress = to.params.address));
        }
        next();
    },
    async created() {
        this.client = await clientPromise;

        if (!this.nextAddress) {
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
        async checkForcedExitEligiblity() {
            const isEligible = await isEligibleForForcedExit(this.address);
            return isEligible;
        },
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.hash);
        },
        async update() {
            // Used to tackle races when
            // the page updates, but this async function
            // tries to update the page, according to the previous address
            const addressAtBeginning = this.nextAddress;

            const account = await this.client.getAccount(addressAtBeginning);
            this.verifiedNonce = account.verified.nonce;
            this.committedNonce = account.committed.nonce;
            const balances = accountStateToBalances(account);
            this.balances = balances.map((bal) => ({
                name: bal.tokenSymbol,
                value: bal.balance
            }));

            const offset = (this.currentPage - 1) * this.rowsPerPage;
            const limit = this.rowsPerPage;

            // maybe load the requested page
            if (this.pagesOfTransactions[this.currentPage] == undefined) {
                this.pagesOfTransactions[this.currentPage] = await this.client.transactionsList(
                    addressAtBeginning,
                    offset,
                    limit
                );
            }

            this.eligibleForForcedExit = await this.checkForcedExitEligiblity();

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

            if (this.nextAddress === addressAtBeginning) {
                this.loading = false;
                this.accountId = account.id;
            }
        },
        loadNewTransactions() {
            this.totalRows = 0;
            this.pagesOfTransactions = {};
            this.load();
        }
    },
    computed: {
        alternativeWithdrawUrl() {
            if (store.network === 'rinkeby' || store.network === 'ropsten') {
                return `https://withdraw-${store.network}.zksync.dev`;
            } else {
                return `https://withdraw.zksync.io`;
            }
        },
        alternativeWithdrawAddressLink() {
            let baseUrl = this.alternativeWithdrawUrl;

            return `${baseUrl}?address=${this.address}`;
        },
        address() {
            return this.$route.params.address;
        },
        accountDataProps() {
            const dataProps = [
                this.addressEntry,
                this.accountIdEntry,
                this.verifiedNonceEntry,
                this.committedNonceEntry
            ];
            return dataProps;
        },
        addressEntry() {
            return makeEntry('Address').innerHTML(this.address).copyable().tooltipRight(true);
        },
        accountIdEntry() {
            return makeEntry('Account Id').innerHTML(this.accountId);
        },
        verifiedNonceEntry() {
            return makeEntry('Verified nonce').innerHTML(this.verifiedNonce);
        },
        committedNonceEntry() {
            return makeEntry('Committed nonce').innerHTML(this.committedNonce);
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
                    text: 'Account ' + this.address,
                    active: true
                }
            ];
        },

        transactionProps() {
            return this.transactions.map((tx) => {
                if (tx.type == 'Withdraw') {
                    tx.type = 'Withdrawal';
                }

                return {
                    ...getTxEntries(tx),
                    success: tx.success,
                    fromAddr: tx.from,
                    toAddr: tx.to,
                    hash: tx.hash
                };
            });
        },
        transactionFields() {
            if (this.transactionProps && this.transactionProps.length) {
                return Object.keys(this.transactionProps[0]).filter(
                    (k) => !['hash', 'fromAddr', 'toAddr', 'success'].includes(k)
                );
            }
            return [];
        }
    },
    components
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
    font-size: 1em;
}
.bigger-text {
    font-size: 1.05em;
}
.brown {
    color: #aa935d;
}

.alternativeWithdrawMsg {
    padding-top: 20px;
    font-size: 15px;
}
</style>
