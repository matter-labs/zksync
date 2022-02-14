<template>
    <div>
        <br />
        <b-container>
            <div v-if="loading">
                <h5 class="mt-3">Transaction data</h5>
                <img src="./assets/loading.gif" width="100" height="100" />
            </div>
            <div v-else-if="transactionExists == false">
                <h5 class="mt-3">
                    Can't find transaction <code> {{ tx_hash }} </code>
                </h5>
            </div>
            <div v-else>
                <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
                <h5 class="mt-3">Transaction data</h5>
                <b-card no-body class="table-margin-hack">
                    <b-table responsive thead-class="displaynone" :items="props">
                        <template v-slot:cell(value)="data">
                            <Entry class="normalize-text" v-if="data.item['name'] == 'From'" :value="data.item.value" />
                            <Entry
                                class="normalize-text"
                                v-else-if="data.item['name'] == 'To'"
                                :value="data.item.value"
                            />
                            <Entry
                                class="normalize-text"
                                v-else-if="data.item['name'] == 'Account'"
                                :value="data.item.value"
                            />
                            <Entry
                                class="normalize-text"
                                v-else-if="data.item['name'] == 'zkSync tx hash'"
                                :value="data.item.value"
                            />
                            <Entry
                                class="normalize-text"
                                v-else-if="data.item['name'] == 'ETH tx hash'"
                                :value="data.item.value"
                            />
                            <Entry v-else-if="data.item.name == 'Status'" :value="data.item.value" />
                            <Entry v-else :value="data.item.value" />
                        </template>
                    </b-table>
                </b-card>
                <br />
            </div>
        </b-container>
    </div>
</template>

<script>
import { clientPromise } from './Client';
import timeConstants from './timeConstants';
import { makeEntry, removeTxHashPrefix } from './utils';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import Entry from './links/Entry.vue';
import { blockchainExplorerTx } from './constants';

import { getTxEntries } from './transactionEntries';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
    Entry
};

export default {
    name: 'transaction',
    data: () => ({
        txData: {},
        status: '',
        intervalHandle: null,
        loading: true,
        transactionExists: true
    }),
    watch: {
        async tx_hash() {
            this.loading = true;
            await this.update();
            this.loading = false;
        }
    },
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

            txData.tokenName = txData.token === -1 ? '' : tokens[txData.token].syncSymbol;
            if (txData.tx_type == 'Deposit' || txData.tx_type == 'FullExit') {
                txData.feeTokenName = 'ETH';
            } else if (txData.tx_type == 'ChangePubKey' || txData.tx_type == 'ChangePubKeyOffchain') {
                // Once upon a time there was no need to pay the fee for the `ChangePubKey` operations,
                // so we need to check if `txData` contains fields associated with fee
                txData.feeTokenName = txData.token === -1 ? '' : tokens[txData.token || 0].syncSymbol;
                txData.fee = txData.fee || 0;
            } else {
                txData.feeTokenName = txData.token === -1 ? '' : tokens[txData.token].syncSymbol;
            }
            txData.amount = txData.amount == 'unknown amount' ? '' : txData.amount;

            if (txData.tx_type == 'Withdraw') {
                txData.tx_type = 'Withdrawal';

                // If the server gives an error, then ignore it.
                try {
                    txData.withdrawalTxHash = await client.withdrawalTxHash(
                        `sync-tx:${removeTxHashPrefix(this.tx_hash)}`
                    );
                } catch (_e) {}
            }

            if (txData.tx_type == 'ForcedExit') {
                // If the server gives an error, then ignore it.
                try {
                    txData.withdrawalTxHash = await client.withdrawalTxHash(
                        `sync-tx:${removeTxHashPrefix(this.tx_hash)}`
                    );
                } catch (_e) {}
            }

            let block = {
                verified_at: null,
                committed_at: null
            };

            if (txData.block_number != -1) {
                const fetchedBlock = await client.getBlock(txData.block_number);
                // Only update block if it's created already.
                // If you query API with a block with a number greater than the last committed block,
                // it will return the last actually committed block (which will be indicated by the block number
                // in the response).
                if (fetchedBlock.block_number == txData.block_number) {
                    block = fetchedBlock;
                }
            }

            if (txData.tx.eth_block_number) {
                txData.numEthConfirmationsToWait = await client.getNumConfirmationsToWait(txData.tx.eth_block_number);
            }

            if (block.verified_at) {
                client.cacher.cacheTransaction(this.tx_hash, txData);
            }

            if (block.verified_at) {
                txData.status = 'Complete';
            } else if (block.committed_at) {
                txData.status = 'Pending';
            } else {
                txData.status = 'Initiated';
            }

            this.txData = txData;
        }
    },
    computed: {
        tx_hash() {
            return this.$route.params.id;
        },
        completeWithdrawalHashEntry() {
            const entry = makeEntry('ETH tx hash');

            if (this.txData.withdrawalTxHash) {
                entry.outterLink(`${blockchainExplorerTx}/${this.txData.withdrawalTxHash}`);
                entry.innerHTML(this.txData.withdrawalTxHash);
                entry.copyable();
            } else {
                entry.innerHTML(`Not yet sent on the chain.`);
                // Also change status for withdrawal without complete L1.
                if (this.txData.status == 'Complete') {
                    this.txData.status = 'Scheduled';
                }
            }

            return entry;
        },
        breadcrumbs() {
            return [
                {
                    text: 'All blocks',
                    to: '/'
                },
                {
                    text: 'Block ' + this.txData.block_number,
                    to: '/blocks/' + this.txData.block_number
                },
                {
                    text: 'Transaction ' + this.tx_hash,
                    active: true
                }
            ];
        },

        hashEntry() {
            if (this.withdrawalTx) {
                return [
                    makeEntry('zkSync tx hash').innerHTML(this.tx_hash).copyable(),
                    this.completeWithdrawalHashEntry
                ];
            }

            const entry = this.onChainTx
                ? makeEntry('ETH tx hash')
                      .outterLink(`${blockchainExplorerTx}/${this.tx_hash}`)
                      .innerHTML(this.tx_hash)
                      .copyable()
                : makeEntry('zkSync tx hash').innerHTML(this.tx_hash).copyable();

            return [entry];
        },
        props() {
            if (Object.keys(this.txData).length == 0) {
                return [];
            }

            return [...this.hashEntry, ...getTxEntries(this.txData)];
        },
        onChainTx() {
            return this.txData.tx_type == 'Deposit' || this.txData.tx_type == 'FullExit';
        },
        withdrawalTx() {
            return this.txData.tx_type == 'Withdrawal' || this.txData.tx_type == 'ForcedExit';
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

.onchain_icon {
    display: inline-block;
    line-height: 1.5em;
    font-weight: bold;
    background: #17a2b8;
    border-radius: 5px;
    padding: 0 0.2em;
    color: white;
}

.layer_icon {
    display: inline-block;
    line-height: 1.5em;
    font-weight: bold;
    background: #17a2b8;
    border-radius: 5px;
    padding: 0 0.2em;
    color: white;
    font-size: 0.8em;
}

.normalize-text {
    font-size: 1em;
}
</style>
