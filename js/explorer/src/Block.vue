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
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <div v-if="loading">
            <img style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">
        </div>
        <div v-else>
            <h5>Block data</h5>
            <b-card no-body>
                <b-table responsive id="my-table" thead-class="hidden_header" :items="props" :busy="isBusy">
                    <template v-slot:cell(value)="data"><span v-html="data.item.value"></span></template>
                </b-table>
            </b-card>
            <br>
            <h5>Transactions in this block</h5>
            <TransactionList :transactions="transactions"></TransactionList>
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
import client from './client';
import { ethers } from 'ethers';
import { readableEther, shortenHash } from './utils';

import TransactionList from './TransactionList.vue';
import SearchField from './SearchField.vue';

const components = {
    TransactionList,
    SearchField,
};

function formatToken(amount, token) {
    return readableEther(amount);
    // if (token == "ETH") {
    //     return ethers.utils.formatEther(amount);
    // }
    // return amount;
}

function defaultTokenSymbol(tokenId) {
    return `erc20_${tokenId}`;
}

function formatDate(date) {
    if (date == null) return '';
    return date.toString().split('T')[0] + " " + date.toString().split('T')[1].split('.')[0];
}

export default {
    name: 'block',
    created() {
        this.update();
    },
    methods: {
        async update() {
            const block = await client.getBlock(this.blockNumber);
            if (!block) return;

            // this.type            = block.type
            this.new_state_root  = block.new_state_root;
            this.commit_tx_hash  = block.commit_tx_hash || '';
            this.verify_tx_hash  = block.verify_tx_hash || '';
            this.committed_at    = block.committed_at;
            this.verified_at     = block.verified_at;
            this.status          = block.verified_at ? 'Verified' : 'Committed';

            let txs = await client.getBlockTransactions(this.blockNumber);
            let tokens = await client.getTokens();
            this.transactions = txs.map((tx, index) => {
                let type = "";
                if (tx.type == "PriorityOp") {
                    type = tx.priority_op.data.type;
                } else if (tx.type == "Tx") {
                    type = tx.tx.type;
                }

                let from = "";
                let to = "";
                let token = "";
                let amount = "";
                let fee = "";
                let from_explorer_link = "";
                let to_explorer_link = "";
                let from_onchain_icon = "";
                let to_onchain_icon = "";

                if (type == "Deposit") {
                    from = shortenHash(tx.priority_op.data.sender, 'unknown sender');
                    to = shortenHash(tx.priority_op.data.account, 'unknown account');
                    from_explorer_link = `${this.blockchain_explorer_address}/${tx.priority_op.data.account}`;
                    to_explorer_link = `${this.routerBase}accounts/${tx.priority_op.data.account}`;
                    from_onchain_icon = `<span class="onchain_icon">onchain</span>`;
                    to_onchain_icon = '';
                    token = tx.priority_op.data.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.priority_op.data.amount, token)} ${token}`;
                    fee = `${formatToken(tx.priority_op.eth_fee, "ETH")} ETH`;
                } else if (type == "Transfer") {
                    from = shortenHash(tx.tx.from, 'unknown from');
                    to = shortenHash(tx.tx.to, 'unknown to');
                    from_explorer_link = `${this.routerBase}accounts/${tx.tx.from}`;
                    to_explorer_link = `${this.routerBase}accounts/${tx.tx.to}`;
                    from_onchain_icon = '';
                    to_onchain_icon = '';
                    token = tx.tx.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.tx.amount, token)} ${token}`;
                    fee = `${formatToken(tx.tx.fee, token)} ${token}`;
                } else if (type == "Withdraw") {
                    from = shortenHash(tx.tx.account, 'unknown account');
                    to = shortenHash(tx.tx.ethAddress, 'unknown ethAddress');
                    from_explorer_link = `${this.routerBase}accounts/${tx.tx.account}`;
                    to_explorer_link = `${this.blockchain_explorer_address}/${tx.tx.ethAddress}`;
                    from_onchain_icon = '';
                    to_onchain_icon = `<span class="onchain_icon">onchain</span>`;
                    token = tx.tx.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.tx.amount, token)} ${token}`;
                    fee = `${formatToken(tx.tx.fee, token)} ${token}`;
                }

                let from_target = from_explorer_link.startsWith('/')
                    ? ''
                    : `_target="_blank" rel="noopener noreferrer"`;

                let to_target = to_explorer_link.startsWith('/')
                    ? ''
                    : `_target="_blank" rel="noopener noreferrer"`;

                this.loading = false;

                return {
                    type: `<b>${type}</b>`,
                    from: `<code><a href="${from_explorer_link}" ${from_target}>${from} ${from_onchain_icon}</a></code>`,
                    to: `<code><a href="${to_explorer_link}" ${to_target}>${to} ${to_onchain_icon}</a></code>`,
                    amount,
                    fee,
                    tx_hash: tx.tx_hash,
                };
            });
        },
    },
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
                { name: 'Block #',          value: `<b>${this.blockNumber}</b>`},
                { name: 'New root hash',    value: `<code>${this.new_state_root}</code>`},
                // { name: 'Transactions',     value: client.TX_PER_BLOCK(), },
                { name: 'Status',           value: this.status, },
                { name: 'Commit tx hash',   value: this.commit_tx_hash
                    ? `<code><a target="blanc" href="${this.blockchain_explorer_tx}/${this.commit_tx_hash}">${this.commit_tx_hash} <span class="onchain_icon">onchain</span></a></code>`
                    : `<b>Not yet sent on the chain.</b>` },
                { name: 'Committed at',     value: formatDate(this.committed_at)},
                { name: 'Verify tx hash',   value: this.verify_tx_hash
                    ? `<code><a target="blanc" href="${this.blockchain_explorer_tx}/${this.verify_tx_hash}">${this.verify_tx_hash} <span class="onchain_icon">onchain</span></a></code>`
                    : `<b>Not yet sent on the chain.</b>` },
                { name: 'Verified at',      value: formatDate(this.verified_at)},
            ];
        },
    },
    data() {
        return {
            new_state_root: null,
            // type:           null,
            commit_tx_hash: null,
            verify_tx_hash: null,
            committed_at:   null,
            verified_at:    null,
            status:         null,
            transactions:   [  ],
            loading:        true,
        };
    },
    components,
};
</script>
