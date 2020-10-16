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
                        <CopyableAddress v-if="data.item.name == 'New root hash'" :address="new_state_root" :linkHtml="data.item.value" />
                        <CopyableAddress v-else-if="data.item.name == 'Commit tx hash'" :address="commit_tx_hash" :linkHtml="data.item.value" />
                        <CopyableAddress v-else-if="data.item.name == 'Verify tx hash'" :address="verify_tx_hash" :linkHtml="data.item.value" />
                        <span v-else-if="data.item.name == 'Status'">
                            <ReadinessStatus :status="data.item.value == 'Pending' ? 1 : 2" />
                            <span v-html="data.item.value" class="mr-1"/>
                            <Question :text="data.item.value" />
                        </span>
                        <span v-else v-html="data.item.value" />
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

import store from './store';
import { shortenHash, formatDate, formatToken } from './utils';

import TransactionList from './TransactionList.vue';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import Question from './Question.vue';
import ReadinessStatus from './ReadinessStatus.vue';
import { clientPromise } from './Client';

const components = {
    TransactionList,
    SearchField,
    CopyableAddress,
    Navbar,
    Question,
    ReadinessStatus,
};

export default {
    name: 'Block',
    created() {
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
        props() {
            return [
                { name: 'Block Number',          value: `${this.blockNumber}`},
                { name: 'Block Size',            value: `${this.block_size}`},
                { name: 'New root hash',         value: `${this.new_state_root}`},
                // { name: 'Transactions',       value: client.TX_PER_BLOCK(), },
                { name: 'Status',                value: this.status, },
                { name: 'Commit tx hash',        value: this.commit_tx_hash
                    ? `<a target="blanc" href="${this.blockchainExplorerTx}/${this.commit_tx_hash}">${this.commit_tx_hash} <i class="fas fa-external-link-alt"></i></a>`
                    : `Not yet sent on the chain.` },
                { name: 'Committed at',          value: formatDate(this.committed_at)},
                { name: 'Verify tx hash',        value: this.verify_tx_hash
                    ? `<a target="blanc" href="${this.blockchainExplorerTx}/${this.verify_tx_hash}">${this.verify_tx_hash} <i class="fas fa-external-link-alt"></i></a>`
                    : `Not yet sent on the chain.` },
                { name: 'Verified at',           value: formatDate(this.verified_at)},
            ];
        },
    },
    methods: {
        async update() {
            const client = await clientPromise;

            const block = await client.getBlock(this.blockNumber).catch(() => null);
            console.log({block});
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

            // TODO: Remove the hack to get the amount field in ForceExit operations
            // API needs to be updated
            
            const transactions = await Promise.all(txs.map(async (tx) => {
                const type = tx.op.type;
                let fromAddr = "";
                let toAddr = "";
                let from = "";
                let to = "";
                let token = "";
                let amount = "";
                let fee = "";
                let from_explorer_link = "";
                let to_explorer_link = "";
                let from_onchain_icon = "";
                let to_onchain_icon = "";
                let success = false;
                let created_at = "";

                switch (type) {
                    case "Deposit":
                        fromAddr           = tx.op.priority_op.from;
                        toAddr             = tx.op.priority_op.to;
                        from               = shortenHash(tx.op.priority_op.from, 'unknown sender');
                        to                 = shortenHash(tx.op.priority_op.to, 'unknown account');
                        from_explorer_link = `${this.blockchainExplorerAddress}/${tx.op.priority_op.from}`;
                        to_explorer_link   = `${this.routerBase}accounts/${tx.op.priority_op.to}`;
                        from_onchain_icon  = `<i class="fas fa-external-link-alt"></i>`;
                        to_onchain_icon    = '';
                        token              = tx.op.priority_op.token;
                        token              = tokens[token].syncSymbol;
                        amount             = `${formatToken(tx.op.priority_op.amount || 0, token)} ${token}`;
                        success            = tx.success;
                        created_at         = tx.created_at;
                        fee                = '';
                        break;
                    case "Transfer":
                        fromAddr           = tx.op.from;
                        toAddr             = tx.op.to;
                        from               = shortenHash(tx.op.from, 'unknown from');
                        to                 = shortenHash(tx.op.to, 'unknown to');
                        from_explorer_link = `${this.routerBase}accounts/${tx.op.from}`;
                        to_explorer_link   = `${this.routerBase}accounts/${tx.op.to}`;
                        from_onchain_icon  = '';
                        to_onchain_icon    = '';
                        token              = tx.op.token;
                        token              = tokens[token].syncSymbol;
                        amount             = `${formatToken(tx.op.amount, token)} ${token}`;
                        fee                = `${formatToken(tx.op.fee, token)} ${token}`;
                        success            = tx.success;
                        created_at         = tx.created_at;
                        break;
                    case "ChangePubKey":
                        fromAddr           = tx.op.account;
                        toAddr             = tx.op.newPkHash;
                        from               = shortenHash(tx.op.account, 'unknown account address');
                        to                 = shortenHash(tx.op.newPkHash, 'unknown pubkey hash');
                        from_explorer_link = `${this.routerBase}accounts/${tx.op.account}`;
                        to_explorer_link   = ``;
                        from_onchain_icon  = '';
                        to_onchain_icon    = '';
                        token              = tx.op.feeToken;
                        token              = token == null ? '' : tokens[token].syncSymbol;
                        amount             = '';
                        fee                = tx.op.fee == null ? '' :`${formatToken(tx.op.fee, token)} ${token}`;
                        success            = tx.success;
                        created_at         = tx.created_at;
                        break;
                    case "Withdraw":
                        fromAddr           = tx.op.from;
                        toAddr             = tx.op.to;
                        from               = shortenHash(tx.op.from, 'unknown account');
                        to                 = shortenHash(tx.op.to, 'unknown ethAddress');
                        from_explorer_link = `${this.routerBase}accounts/${tx.op.from}`;
                        to_explorer_link   = `${this.blockchainExplorerAddress}/${tx.op.to}`;
                        from_onchain_icon  = '';
                        to_onchain_icon    = `<i class="fas fa-external-link-alt"></i>`;
                        token              = tx.op.token;
                        token              = tokens[token].syncSymbol;
                        amount             = `${formatToken(tx.op.amount || 0, token)} ${token}`;
                        fee                = `${formatToken(tx.op.fee, token)} ${token}`;
                        success            = tx.success;
                        created_at         = tx.created_at;
                        break;
                    case "ForcedExit":
                        fromAddr           = tx.op.target;
                        toAddr             = tx.op.target;
                        from               = shortenHash(tx.op.target, 'unknown account');
                        to                 = shortenHash(tx.op.target, 'unknown ethAddress');
                        from_explorer_link = `${this.routerBase}accounts/${tx.op.target}`;
                        to_explorer_link   = `${this.blockchainExplorerAddress}/${tx.op.target}`;
                        from_onchain_icon  = '';
                        to_onchain_icon    = `<i class="fas fa-external-link-alt"></i>`;
                        token              = tx.op.token;
                        token              = tokens[token].syncSymbol;
                        amount             = (await client.searchTx(tx.tx_hash)).amount;
                        amount             = amount == "unknown amount" ? 0 : amount;
                        amount             = `${formatToken(amount || 0, token)} ${token}`;
                        fee                = `${formatToken(tx.op.fee, token)} ${token}`;
                        success            = tx.success;
                        created_at         = tx.created_at;
                        break;
                    case "FullExit":
                        fromAddr           = tx.op.priority_op.eth_address;
                        toAddr             = tx.op.priority_op.eth_address;
                        from               = shortenHash(tx.op.priority_op.eth_address, 'unknown account address');
                        to                 = shortenHash(tx.op.priority_op.eth_address, 'unknown account address');
                        from_explorer_link = `${this.routerBase}accounts/${tx.op.priority_op.eth_address}`;
                        to_explorer_link   = `${this.blockchainExplorerAddress}/${tx.op.priority_op.eth_address}`;
                        from_onchain_icon  = `<i class="fas fa-external-link-alt"></i>`;
                        to_onchain_icon    = `<i class="fas fa-external-link-alt"></i>`;
                        token              = tx.op.priority_op.token;
                        token              = tokens[token].syncSymbol;
                        amount             = `${formatToken(tx.op.withdraw_amount || 0, token)} ${token}`;
                        success            = tx.success;
                        created_at         = tx.created_at;
                        fee                = '';
                        break;
                    default:
                        throw new Error('switch reached default');
                }

                const from_target 
                    = from_explorer_link.startsWith('/') ? ''
                    : from_explorer_link == '' ? ''
                    : `target="_blank" rel="noopener noreferrer"`;

                const to_target 
                    = to_explorer_link.startsWith('/') ? ''
                    : to_explorer_link == '' ? ''
                    : `target="_blank" rel="noopener noreferrer"`;

                return {
                    tx_hash: tx.tx_hash,
                    type: `${type}`,
                    from: `<a href="${from_explorer_link}" ${from_target}>${from} ${from_onchain_icon}</a>`,
                    to: `<a href="${to_explorer_link}" ${to_target}>${to} ${to_onchain_icon}</a>`,
                    fromAddr,
                    toAddr,
                    amount,
                    fee,
                    success,
                    created_at: formatDate(created_at),
                };
            }));

            this.transactions = transactions.filter(tx => tx.success);
            this.loadingStatus = 'ready';
        },
    },
    components,
};
</script>

<style>
</style>
