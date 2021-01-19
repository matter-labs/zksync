<template>
    <div>
        <br />
        <b-container>
            <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
            <div v-if="loadingStatus == 'loading'">
                <img style="margin-right: 1.5em" src="./assets/loading.gif" width="100em" />
            </div>
            <div v-else-if="loadingStatus == 'not committed'">This block is not committed yet.</div>
            <div v-else>
                <h5>Block data</h5>
                <b-card no-body>
                    <b-table
                        responsive
                        id="my-table"
                        thead-class="displaynone"
                        :items="props"
                        :busy="isBusy"
                        class="nowrap"
                    >
                        <template v-slot:cell(value)="data">
                            <Entry v-if="data.item.name == 'New root hash'" :value="data.item.value" />
                            <Entry v-else-if="data.item.name == 'Commit tx hash'" :value="data.item.value" />
                            <Entry v-else-if="data.item.name == 'Verify tx hash'" :value="data.item.value" />
                            <Entry v-else-if="data.item.name == 'Status'" :value="data.item.value" />
                            <Entry v-else :value="data.item.value" />
                        </template>
                    </b-table>
                </b-card>
                <br />
                <h5>Transactions in this block</h5>
                <TransactionList :transactions="transactions"></TransactionList>
            </div>
        </b-container>
    </div>
</template>

<script>
import { formatDate, makeEntry, readyStateFromString } from './utils';

import TransactionList from './TransactionList.vue';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import { clientPromise } from './Client';
import Entry from './links/Entry';

import { blockchainExplorerTx } from './constants';

import { getTxEntries } from './blockTxEnries';

const components = {
    TransactionList,
    SearchField,
    CopyableAddress,
    Navbar,
    Entry
};

export default {
    name: 'Block',
    mounted() {
        this.update();
    },
    data: () => ({
        new_state_root: null,
        commit_tx_hash: null,
        verify_tx_hash: null,
        committed_at: null,
        verified_at: null,
        status: null,
        transactions: [],
        loadingStatus: 'loading'
    }),
    computed: {
        isBusy: () => false,
        blockNumber() {
            return this.$route.params.blockNumber;
        },
        breadcrumbs() {
            return [
                {
                    text: 'All blocks',
                    to: '/'
                },
                {
                    text: 'Block ' + this.blockNumber,
                    active: true
                }
            ];
        },
        rows() {
            return this.items.length;
        },
        blockNumberEntry() {
            return makeEntry('Block Number').innerHTML(this.blockNumber);
        },
        blockSizeEntry() {
            return makeEntry('Block Size').innerHTML(this.block_size);
        },
        rootHashEntry() {
            return makeEntry('New root hash').innerHTML(this.new_state_root).copyable();
        },
        statusEntry() {
            return makeEntry('Status').status(readyStateFromString(this.status)).innerHTML(this.status);
        },
        commitHashEntry() {
            const entry = makeEntry('Commit tx hash').copyable();

            if (this.commit_tx_hash) {
                entry.outterLink(`${blockchainExplorerTx}/${this.commit_tx_hash}`);
                entry.innerHTML(`${this.commit_tx_hash}`);
            } else {
                entry.innerHTML(`Not yet sent on the chain.`);
            }

            return entry;
        },
        commitedAtEntry() {
            return makeEntry('Committed at').innerHTML(formatDate(this.committed_at));
        },
        verifyHashEntry() {
            const entry = makeEntry('Verify tx hash');

            if (this.verify_tx_hash) {
                entry.outterLink(`${blockchainExplorerTx}/${this.verify_tx_hash}`);
                entry.innerHTML(`${this.verify_tx_hash}`);
                entry.copyable();
            } else {
                entry.innerHTML(`Not yet sent on the chain.`);
            }

            return entry;
        },
        verifiedAtEntry() {
            return makeEntry('Verified at').innerHTML(formatDate(this.verified_at));
        },
        props() {
            return [
                this.blockNumberEntry,
                this.blockSizeEntry,
                this.rootHashEntry,
                this.statusEntry,
                this.commitHashEntry,
                this.commitedAtEntry,
                this.verifyHashEntry,
                this.verifiedAtEntry
            ];
        }
    },
    methods: {
        async update() {
            const client = await clientPromise;

            const block = await client.getBlock(this.blockNumber).catch(() => null);
            if (!block) {
                this.loadingStatus = 'not committed';
                return;
            }

            if (block.block_number != this.blockNumber) {
                this.loadingStatus = 'not committed';
                return;
            }

            this.new_state_root = block.new_state_root.slice(8);
            this.commit_tx_hash = block.commit_tx_hash || '';
            this.verify_tx_hash = block.verify_tx_hash || '';
            this.committed_at = block.committed_at;
            this.verified_at = block.verified_at;
            this.status = block.verified_at ? 'Verified' : 'Pending';
            this.block_size = block.block_size;

            const txs = await client.getBlockTransactions(this.blockNumber, block);
            const tokens = await client.tokensPromise;

            const transactions = await Promise.all(
                txs.map(async (tx) => {
                    return {
                        ...(await getTxEntries(tx, tokens, client)),
                        success: tx.success
                    };
                })
            );

            this.transactions = transactions.filter((tx) => tx.success);
            this.loadingStatus = 'ready';
        }
    },
    components
};
</script>

<style></style>
