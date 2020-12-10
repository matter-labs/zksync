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
                            <EntryComponent
                                class="normalize-text"
                                v-if="data.item['name'] == 'From'"
                                :value="data.item.value"
                            />
                            <EntryComponent
                                class="normalize-text"
                                v-else-if="data.item['name'] == 'To'"
                                :value="data.item.value"
                            />
                            <EntryComponent
                                class="normalize-text"
                                v-else-if="data.item['name'] == 'Account'"
                                :value="data.item.value"
                            />
                            <EntryComponent
                                class="normalize-text"
                                v-else-if="['zkSync tx hash', 'ETH Tx hash'].includes(data.item['name'])"
                                :value="data.item.value"
                            />
                            <span v-else-if="data.item.name == 'Status'">
                                <ReadinessStatus :status="readyStateFromString(data.item.value.innerHTML)" />
                                <span v-html="data.item.value.innerHTML" class="mr-1" />
                                <Question :text="data.item.value.innerHTML" />
                            </span>
                            <span v-else v-html="data.item.value.innerHTML" />
                        </template>
                    </b-table>
                </b-card>
                <br />
            </div>
        </b-container>
    </div>
</template>

<script>
import { formatDate, formatToken, makeEntry, blockchainExplorerToken } from './utils';
import { clientPromise } from './Client';
import timeConstants from './timeConstants';

import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import Question from './Question.vue';
import ReadinessStatus from './ReadinessStatus.vue';
import EntryComponent from './links/Entry';

import { blockchainExplorerTx, blockchainExplorerAddress } from './constants';
import { BigNumber } from 'ethers';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
    Question,
    ReadinessStatus,
    EntryComponent
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
        readyStateFromString(s) {
            return {
                Rejected: -1,
                Initiated: 0,
                Pending: 1,
                Complete: 2
            }[s];
        },
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

            txData.status = block.verified_at ? `Complete` : block.committed_at ? `Pending` : `Initiated`;

            this.txData = txData;
        }
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
                    to: '/blocks/' + this.txData.block_number
                },
                {
                    text: 'Transaction ' + this.tx_hash,
                    active: true
                }
            ];
        },
        hashEntry() {
            const entry = this.onChainTx
                ? makeEntry('ETH Tx hash')
                      .outterLink(`${blockchainExplorerTx}/${this.tx_hash}`)
                      .innerHTML(this.tx_hash)
                : makeEntry('zkSync tx hash').innerHTML(this.tx_hash);

            return entry.copyable();
        },
        fromLinkEntry() {
            const entry = makeEntry('From').copyable();

            if (this.txData.tx_type == 'Deposit') {
                entry.outterLink(`${blockchainExplorerAddress}/${this.txData.from}`);
            } else {
                entry.localLink(`/accounts/${this.txData.from}`);
            }

            if (
                this.txData.tx_type == 'Withdrawal' ||
                this.txData.tx_type == 'FullExit' ||
                this.txData.tx_type == 'ForcedExit'
            ) {
                entry.layer(2);
            }
            if (this.txData.tx_type == 'Deposit') {
                entry.layer(1);
            }

            if (this.txData.tx_type == 'ChangePubKey') {
                entry.rename('Account');
            }

            entry.innerHTML(this.txData.from);

            return entry;
        },
        toLinkEntry() {
            const entry = makeEntry('To').copyable();

            if (this.txData.tx_type == 'Withdrawal') {
                entry.outterLink(blockchainExplorerToken(this.txData.tokenName, this.txData.to));
            } else {
                entry.localLink(`/accounts/${this.txData.to}`);
            }

            if (
                this.txData.tx_type == 'Withdrawal' ||
                this.txData.tx_type == 'FullExit' ||
                this.txData.tx_type == 'ForcedExit'
            ) {
                entry.layer(1);
            }
            if (this.txData.tx_type == 'Deposit') {
                entry.layer(2);
            }

            return entry.innerHTML(this.txData.to);
        },
        typeEntry() {
            return makeEntry('Type').innerHTML(this.txData.tx_type);
        },
        statusEntry() {
            const entry = makeEntry('Status');

            if (this.txData.fail_reason) {
                entry.innerHTML('Rejected');
            } else {
                entry.innerHTML(this.txData.status);
            }

            return entry;
        },
        feeEntry() {
            const fee = this.txData.fee || 0;

            try {
                const feeBN = BigNumber.from(fee);
                if (feeBN.eq('0')) {
                    return makeEntry('Fee').innerHTML(
                        '<i>This transaction is a part of a batch. The fee was payed in another transaction.</i>'
                    );
                }
            } catch {
                return makeEntry('Fee');
            }

            return makeEntry('Fee').innerHTML(
                `${this.txData.feeTokenName} ${formatToken(fee, this.txData.feeTokenName)}`
            );
        },
        createdAtEntry() {
            return makeEntry('Created at').innerHTML(formatDate(this.txData.created_at));
        },
        amountEntry() {
            return makeEntry('Amount').innerHTML(
                `${this.txData.tokenName} ${formatToken(this.txData.amount || 0, this.txData.feeTokenName)}`
            );
        },
        newSignerPubKeyHashEntry() {
            if (this.txData.tx_type == 'ChangePubKey') {
                return makeEntry('New signer key hash').innerHTML(`${this.txData.to.replace('sync:', '')}`);
            } else {
                // This entry won't be used for any tx_type
                // except for ChangePubKey anyway
                return '';
            }
        },
        props() {
            if (Object.keys(this.txData).length == 0) return [];

            const rows = [];

            if (this.txData.nonce != -1 && (this.txData.nonce || this.txData === 0)) {
                rows.push(makeEntry('Nonce').innerHTML(this.txData.nonce));
            }

            if (this.txData.numEthConfirmationsToWait) {
                rows.push(makeEntry('Eth confirmations').innerHTML(this.txData.numEthConfirmationsToWait));
            }

            if (this.txData.fail_reason) {
                rows.push(makeEntry('Rejection reason:').innerHTML(this.txData.fail_reason));
            }

            if (this.txData.tx_type == 'ChangePubKey') {
                return [
                    this.hashEntry,
                    this.typeEntry,
                    this.statusEntry,
                    this.fromLinkEntry,
                    this.feeEntry,
                    this.newSignerPubKeyHashEntry,
                    this.createdAtEntry,
                    ...rows
                ];
            }

            if (this.txData.tx_type == 'Deposit' || this.txData.tx_type == 'FullExit') {
                return [
                    this.hashEntry,
                    this.typeEntry,
                    this.statusEntry,
                    this.fromLinkEntry,
                    this.toLinkEntry,
                    this.amountEntry,
                    ...rows
                ];
            }

            return [
                this.hashEntry,
                this.typeEntry,
                this.statusEntry,
                this.fromLinkEntry,
                this.toLinkEntry,
                this.amountEntry,
                this.feeEntry,
                this.createdAtEntry,
                ...rows
            ];
        },
        onChainTx() {
            return this.txData.tx_type == 'Deposit' || this.txData.tx_type == 'FullExit';
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
