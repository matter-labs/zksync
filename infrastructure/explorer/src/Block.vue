<template>
<div>
    <Navbar />
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <div v-if="loadingStatus == 'loading'">
            <img style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">
        </div>
        <div v-else-if="loadingStatus == 'not committed'">
            This block is not committed yet.
        </div>
        <div v-else>
            <h5>Block data</h5>
            <b-card no-body>
                <b-table responsive id="my-table" thead-class="displaynone" :items="props" :busy="isBusy" class="nowrap">
                    <template v-slot:cell(value)="data">
                        <Entry 
                            v-if="data.item.name == 'New root hash'" 
                            :value="data.item.value"
                        />
                        <Entry 
                            v-else-if="data.item.name == 'Commit tx hash'" 
                            :value="data.item.value"
                        />
                        <Entry 
                            v-else-if="data.item.name == 'Verify tx hash'" 
                            :value="data.item.value"
                        />
                        <span v-else-if="data.item.name == 'Status'">
                            <ReadinessStatus :status="data.item.value.innerHTML == 'Pending' ? 1 : 2" />
                            <span v-html="data.item.value.innerHTML" class="mr-1"/>
                            <Question :text="data.item.value.innerHTML" />
                        </span>
                        <Entry
                            v-else
                            :value="data.item.value"
                        />
                    </template>
                </b-table>
            </b-card>
            <br>
            <h5>Transactions in this block</h5>
            <TransactionList :transactions="transactions"></TransactionList>
        </div>
    </b-container>
</div>
</template>

<script>

import { shortenHash, formatDate, makeEntry } from './utils';

import TransactionList from './TransactionList.vue';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import Question from './Question.vue';
import ReadinessStatus from './ReadinessStatus.vue';
import { clientPromise } from './Client';
import Entry from './links/Entry';

import { 
    blockchainExplorerTx,
    blockchainExplorerAddress,
} from './constants';

import  {
    getTxFromAddress,
    getTxFromFallbackValue,
    getTxToAddress,
    getTxToFallbackValue,
    getTxToken,
    getTxAmount,
    getTxFee,
} from './blockUtils';

const components = {
    TransactionList,
    SearchField,
    CopyableAddress,
    Navbar,
    Question,
    ReadinessStatus,
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
        committed_at:   null,
        verified_at:    null,
        status:         null,
        transactions:   [  ],
        loadingStatus:  'loading',
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
                    text: 'Block '+this.blockNumber,
                    active: true
                },
            ];
        },
        rows() {
            return this.items.length;
        },
        commitHashEntry() {
            const entry = makeEntry('Commit tx hash').copyable();

            if(this.commit_tx_hash) {
                entry.outterLink(`${blockchainExplorerTx}/${this.commit_tx_hash}`);
                entry.innerHTML(`${this.commit_tx_hash}`);
            } else {
                entry.innerHTML(`Not yet sent on the chain.`);
            }

            return entry;
        },
        verifyHashEntry() {
            const entry = makeEntry('Verify tx hash').copyable();

            if(this.verify_tx_hash) {
                entry.outterLink(`${blockchainExplorerTx}/${this.verify_tx_hash}`);
                entry.innerHTML(`${this.verify_tx_hash}`);
            } else {
                entry.innerHTML(`Not yet sent on the chain.`);
            }

            return entry;
        },
        rootHashEntry() {
            return makeEntry('New root hash')
                .innerHTML(this.new_state_root)
                .copyable();
        },
        props() {
            return [
                makeEntry('Block Number').innerHTML(this.blockNumber),
                makeEntry('Block Size').innerHTML(this.block_size),
                this.rootHashEntry,
                makeEntry('Status').innerHTML(this.status),
                this.commitHashEntry,
                makeEntry('Committed at').innerHTML(formatDate(this.committed_at)),
                this.verifyHashEntry,
                makeEntry('Verified at').innerHTML(formatDate(this.verified_at)),
            ];
        },
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

            this.new_state_root  = block.new_state_root.slice(8);
            this.commit_tx_hash  = block.commit_tx_hash || '';
            this.verify_tx_hash  = block.verify_tx_hash || '';
            this.committed_at    = block.committed_at;
            this.verified_at     = block.verified_at;
            this.status          = block.verified_at ? 'Verified' : 'Pending';
            this.block_size      = block.block_size;

            const txs = await client.getBlockTransactions(this.blockNumber);
            const tokens = await client.tokensPromise;

            const transactions = await Promise.all(txs.map(async (tx) => {
                return {
                    ...this.txEntries(tx, tokens, client),
                    success: tx.success
                };
            }));

            this.transactions = transactions.filter(tx => tx.success);
            this.loadingStatus = 'ready';
        },
        txHashEntry(tx) {
            const entry = makeEntry('Tx Hash');
            entry.localLink(`/transactions/${tx.tx_hash}`);
            
            entry.innerHTML(shortenHash(tx.tx_hash));
            return entry;
        },  
        txTypeEntry(tx) {   
            return makeEntry('Type').innerHTML(tx.op.type);
        },
        txFromEntry(tx) {
            const entry = makeEntry('From');

            const fromAddress = getTxFromAddress(tx);
            const fallback = getTxFromFallbackValue(tx);

            if(tx.op.type === 'Deposit') {
                entry.outterLink(`${blockchainExplorerAddress}/${fromAddress}`);
            } else {
                entry.localLink(`${this.routerBase}accounts/${fromAddress}`);
            }

            entry.innerHTML(shortenHash(fromAddress, fallback));       
            return entry;
        },

        txToEntry(tx) {
            const entry = makeEntry('To');

            if(tx.op.type === 'ChangePubKey') {
                return entry; 
            }

            const toAddress = getTxToAddress(tx);
            const fallback = getTxToFallbackValue(tx);

            const onChainWithdrawals = [
                'Withdraw',
                'ForcedExit',
                'FullExit'
            ];
            
            if(onChainWithdrawals.includes(tx.op.type)) {
                entry.outterLink(`${blockchainExplorerAddress}/${toAddress}`);
            } else {
                entry.localLink(`${this.routerBase}accounts/${toAddress}`);
            }
            
            entry.innerHTML(shortenHash(toAddress, fallback));

            return entry;
        },

        txAmountEntry(tx, tokenSymbol, client) {
            return makeEntry('Amount')
                .innerHTML(getTxAmount(tx, tokenSymbol, client));
        },
        txFeeEntry(tx, tokenSymbol) {
            return makeEntry('Fee')
                .innerHTML(getTxFee(tx, tokenSymbol));
        },
        txCreatedAtEntry(tx) {
            return makeEntry('Created at')
                .innerHTML(formatDate(tx.created_at));
        },
        txEntries(tx, tokens, client) {
            const tokenSymbol = tokens[getTxToken(tx)].syncSymbol;

            const txHash = this.txHashEntry(tx);
            const type = this.txTypeEntry(tx);
            const from = this.txFromEntry(tx);
            const to = this.txToEntry(tx);
            const amount = this.txAmountEntry(tx, tokenSymbol, client);
            const fee = this.txFeeEntry(tx, tokenSymbol);
            const createdAt = this.txCreatedAtEntry(tx);

            return {
                txHash,
                type,
                from,
                to,
                amount,
                fee,
                createdAt
            };
        }
    },
    components,
};
</script>

<style>
</style>
