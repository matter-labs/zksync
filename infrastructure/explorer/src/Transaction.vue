<template>
<div>
    <Navbar />
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
                <b-table responsive thead-class="displaynone" :items="props">
                    <template v-slot:cell(value)="data">
                        <CopyableAddress class="normalize-text"
                            v-if="data.item['name'] == 'From'" 
                            :address="txData.from" 
                            :linkHtml="data.item['value']"
                        />
                        <CopyableAddress class="normalize-text" 
                            v-else-if="data.item['name'] == 'To'" 
                            :address="txData.to" 
                            :linkHtml="data.item['value']"
                        />
                        <CopyableAddress class="normalize-text" 
                            v-else-if="data.item['name'] == 'Account'" 
                            :address="txData.from" 
                            :linkHtml="data.item['value']"
                        />
                        <CopyableAddress class="normalize-text" 
                            v-else-if="['zkSync tx hash', 'ETH Tx hash'].includes(data.item['name'])" 
                            :address="tx_hash" 
                            :linkHtml="data.item['value']"
                        />
                        <span v-else-if="data.item.name == 'Status'">
                            <ReadinessStatus :status="readyStateFromString(data.item.value)" />
                            <span v-html="data.item.value" class="mr-1"/>
                            <Question :text="data.item.value" />
                        </span>
                        <span v-else v-html="data.item['value']" />
                    </template>
                </b-table>
            </b-card>
            <br>
        </div>
    </b-container>
</div>
</template>

<script>

import store from './store';
import { formatDate, formatToken } from './utils';
import { clientPromise } from './Client';
import timeConstants from './timeConstants';

import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import Question from './Question.vue';
import ReadinessStatus from './ReadinessStatus.vue';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
    Question,
    ReadinessStatus,
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
        readyStateFromString(s) {
            return {
                "Rejected": -1,
                "Initiated": 0,
                "Pending": 1,
                "Complete": 2,
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

            txData.tokenName = txData.token === -1 ? "" : tokens[txData.token].syncSymbol;
            if (txData.tx_type  == "Deposit" || txData.tx_type == "FullExit") {
                txData.feeTokenName = "ETH";
            } else if(txData.tx_type  == "ChangePubKey" || txData.tx_type == "ChangePubKeyOffchain") {
                // Once upon a time there was no need to pay the fee for the `ChangePubKey` operations,
                // so we need to check if `txData` contains fields associated with fee
                txData.feeTokenName = txData.token === -1 ? "" : tokens[txData.token || 0].syncSymbol;
                txData.fee = txData.fee || 0;
            }
            else {
                txData.feeTokenName = txData.token === -1 ? "" : tokens[txData.token].syncSymbol;
            }
            txData.amount = txData.amount == "unknown amount" ? "" : txData.amount;

            let block = {
                verified_at: null,
                committed_at: null,
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

            txData.status = block.verified_at ? `Complete`
                           : block.committed_at ? `Pending`
                           : `Initiated`;

            if (txData.tx_type == 'Withdraw') {
                txData.tx_type = 'Withdrawal';
            }
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

            const tx_hash = this.txData.tx_type  == "Deposit" || this.txData.tx_type == "FullExit"
                ? `<a href="${this.blockchainExplorerTx}/${this.tx_hash}">${this.tx_hash}</a> <i class="fas fa-external-link-alt"></i>`
                : `${this.tx_hash}`;

            const link_from 
                = this.txData.tx_type == 'Deposit' ? `${this.blockchainExplorerAddress}/${this.txData.from}`
                : `${this.routerBase}accounts/${this.txData.from}`;

            const link_to 
                = this.txData.tx_type == 'Withdrawal' ? this.blockchainExplorerToken(this.txData.tokenName, this.txData.to)
                : this.txData.tx_type == 'ChangePubKey' ? ''
                : `${this.routerBase}accounts/${this.txData.to}`;

            const onchain_from
                = this.txData.tx_type == 'Deposit' ? ` <i class="fas fa-external-link-alt"></i>`
                : '';

            const onchain_to
                = this.txData.tx_type == 'Withdrawal' ? ` <i class="fas fa-external-link-alt"></i>`
                : '';

            const target_from
                = this.txData.tx_type == 'Deposit' ? `target="_blank" rel="noopener noreferrer"`
                : '';

            const target_to
                = this.txData.tx_type == 'Withdrawal' ? `target="_blank" rel="noopener noreferrer"`
                : '';

            const layer_from 
                = this.txData.tx_type == 'Withdrawal' ? `<span class='layer_icon'>L2</span>`
                : this.txData.tx_type == 'Deposit'    ? `<span class='layer_icon'>L1</span>`
                : this.txData.tx_type == 'FullExit'   ? `<span class='layer_icon'>L2</span>`
                : '';

            const layer_to 
                = this.txData.tx_type == 'Withdrawal' ? `<span class='layer_icon'>L1</span>`
                : this.txData.tx_type == 'Deposit'    ? `<span class='layer_icon'>L2</span>`
                : this.txData.tx_type == 'FullExit'   ? `<span class='layer_icon'>L1</span>`
                : '';

            const rows = this.txData.tx_type == "ChangePubKey"
                ? [
                    { name: 'zkSync tx hash',           value: tx_hash},
                    { name: "Type",                     value: `${this.txData.tx_type}`   },
                    { name: "Status",                   value: `${this.txData.status}` },
                    { name: "Account",                  value: `<a ${target_from} href="${link_from}">${this.txData.from}${onchain_from}</a>` },
                    { name: "fee",                      value: `${this.txData.feeTokenName} ${formatToken(this.txData.fee || 0, this.txData.feeTokenName)}` },
                    { name: "New signer key hash",      value: `${this.txData.to.replace('sync:', '')}`},
                    { name: "Created at",               value: formatDate(this.txData.created_at) },
                ]
                : this.txData.tx_type == "Deposit" || this.txData.tx_type == "FullExit"
                ? [
                    { name: 'ETH Tx hash',    value: tx_hash},
                    { name: "Type",           value: `${this.txData.tx_type}`   },
                    { name: "Status",         value: `${this.txData.status}` },
                    { name: "From",           value: `${layer_from} <a ${target_from} href="${link_from}">${this.txData.from}${onchain_from}</a>` },
                    { name: "To",             value: `${layer_to} <a ${target_to} href="${link_to}">${this.txData.to}${onchain_to}</a>`      },
                    { name: "Amount",         value: `${this.txData.tokenName} ${formatToken(this.txData.amount || 0, this.txData.tokenName)}`    },
                ]
                : [
                    { name: 'zkSync tx hash', value: tx_hash},
                    { name: "Type",           value: `${this.txData.tx_type}`   },
                    { name: "Status",         value: `${this.txData.status}` },
                    { name: "From",           value: `${layer_from} <a ${target_from} href="${link_from}">${this.txData.from}${onchain_from}</a>` },
                    { name: "To",             value: `${layer_to} <a ${target_to} href="${link_to}">${this.txData.to}${onchain_to}</a>`      },
                    { name: "Amount",         value: `${this.txData.tokenName} ${formatToken(this.txData.amount || 0, this.txData.tokenName)}`    },
                    { name: "fee",            value: `${this.txData.feeTokenName} ${formatToken(this.txData.fee, this.txData.tokenName)}` },
                    { name: "Created at",     value: formatDate(this.txData.created_at) },
                ];

            if (this.txData.nonce != -1) {
                rows.push({ name: "Nonce",      value: this.txData.nonce });
            }

            if (this.txData.numEthConfirmationsToWait) {
                rows.push({ name: 'Eth confirmations', value: this.txData.numEthConfirmationsToWait });
            }

            if (this.txData.fail_reason) {
                rows.push({ name: "Rejection reason:", value: `${this.txData.fail_reason}` });
                rows.find(r => r.name == 'Status').value = 'Rejected';
            }

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

.layer_icon {
    display: inline-block;
    line-height: 1.5em;
    font-weight: bold;
    background: #17a2b8;
    border-radius: 5px;
    padding: 0 .2em;
    color: white;
    font-size: 0.8em;
}

.normalize-text {
    font-size: 1.0em;
}
</style>
