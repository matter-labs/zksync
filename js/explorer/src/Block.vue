<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand href="/">Matter Network</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <h5>Block data</h5>
        <b-card no-body>
            <b-table responsive id="my-table" thead-class="hidden_header" :items="props" :busy="isBusy">
                <template v-slot:cell(value)="data"><span v-html="data.item.value"></span></template>
            </b-table>
        </b-card>
        <br>
        <h5>Transactions in this block</h5>
        <transaction-list :transactions="transactions"></transaction-list>
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
import TransactionList from './TransactionList.vue';
import client from './client';
import { ethers } from 'ethers';
import { readableEther } from './utils';

function formatToken(amount, token) {
    return readableEther(amount);
    // if (token == "ETH") {
    //     return ethers.utils.formatEther(amount);
    // }
    // return amount;
}

function formatAddress(address) {
    return address;
    // return `${address.slice(0, 12)}..${address.slice(-12)}`;
}

function defaultTokenSymbol(tokenId) {
    return `erc20_${tokenId}`;
}

function formatDate(date) {
    if (date == null) return '';
    return date.toString().split('T')[0] + " " + date.toString().split('T')[1].split('.')[0]
}

export default {
    name: 'block',
    components: {
        'transaction-list':  TransactionList
    },
    created() {
        this.update();
    },
    methods: {
        async update() {
            this.loading = true;
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
            console.log('block txs:', txs);
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

                if (type == "Deposit") {
                    from = formatAddress(tx.priority_op.data.sender);
                    to = formatAddress(tx.priority_op.data.account);
                    from_explorer_link = `${this.blockchain_explorer_address}/${tx.priority_op.data.account}`;
                    to_explorer_link = `/accounts/${tx.priority_op.data.account}`;
                    token = tx.priority_op.data.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.priority_op.data.amount, token)} ${token}`;
                    fee = `${formatToken(tx.priority_op.eth_fee, "ETH")} ETH`;
                } else if (type == "Transfer") {
                    from = formatAddress(tx.tx.from);
                    to = formatAddress(tx.tx.to);
                    from_explorer_link = `/accounts/${tx.tx.from}`;
                    to_explorer_link = `/accounts/${tx.tx.to}`;
                    token = tx.tx.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.tx.amount, token)} ${token}`;
                    fee = `${formatToken(tx.tx.fee, token)} ${token}`;
                } else if (type == "Withdraw") {
                    from = formatAddress(tx.tx.account);
                    to = formatAddress(tx.tx.eth_address);
                    from_explorer_link = `/accounts/${tx.tx.account}`;
                    to_explorer_link = `${this.blockchain_explorer_address}/${tx.tx.account}`;
                    token = tx.tx.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.tx.amount, token)} ${token}`;
                    fee = `${formatToken(tx.tx.fee, token)} ${token}`;
                }

                return {
                    type: `<b>${type}</b>`,
                    from: `<code><a href="${from_explorer_link}" target="_blanc">${from}</a></code>`,
                    to: `<code><a href="${to_explorer_link}" target="_blanc">${to}</a></code>`,
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
                { name: 'Commit tx hash',   value: `<code><a target="blanc" href="${this.blockchain_explorer_tx}/${this.commit_tx_hash}">${this.commit_tx_hash}</a></code>`, },
                { name: 'Committed at',     value: formatDate(this.committed_at)},
                { name: 'Verify tx hash',   value: `<code><a target="blanc" href="${this.blockchain_explorer_tx}/${this.verify_tx_hash}">${this.verify_tx_hash}</a></code>`, },
                { name: 'Verified at',      value: formatDate(this.verified_at)},
            ];
        }
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
        };
    }
};
</script>
