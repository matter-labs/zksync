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
                <span slot="value" slot-scope="data" v-html="data.value"></span>
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

import store from './store'
import TransactionList from './TransactionList.vue'
import client from './client'
import {ethers} from 'ethers';

function formatToken(amount, token) {
    if (token == "ETH") {
        return ethers.utils.formatEther(amount);
    }
    return amount;
}

function formatAddress(address) {
    // return `${address.slice(0,8)}..${address.slice(36, 42)}`;
    return address;
}

function defaultTokenSymbol(tokenId) {
    return `erc20_${tokenId}`;
}

export default {
    name: 'block',
    components: {
        'transaction-list':  TransactionList
    },
    created() {
        this.update()
    },
    methods: {
        async update() {
            this.loading = true
            const block = await client.getBlock(this.blockNumber)
            if (!block) return

            // this.type            = block.type
            this.new_state_root  = block.new_state_root
            this.commit_tx_hash  = block.commit_tx_hash || ''
            this.verify_tx_hash  = block.verify_tx_hash || ''
            this.committed_at    = block.committed_at
            this.verified_at     = block.verified_at
            this.status          = block.verified_at ? 'Verified' : 'Committed'

            let txs = await client.getBlockTransactions(this.blockNumber)
            let tokens = await client.getTokens();
            this.transactions = txs.map( (tx, index) => {

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

                if (type == "Deposit") {
                    from = formatAddress(tx.priority_op.data.sender);
                    to = formatAddress(tx.priority_op.data.account);
                    token = tx.priority_op.data.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.priority_op.data.amount, token)} ${token}`;
                    fee = `${formatToken(tx.priority_op.eth_fee, "ETH")} ETH`;
                } else if (type == "Transfer") {
                    from = formatAddress(tx.tx.from);
                    to = formatAddress(tx.tx.to);
                    token = tx.tx.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.tx.amount, token)} ${token}`;
                    fee = `${formatToken(tx.tx.fee, token)} ${token}`;
                } else if (type == "Withdraw") {
                    from = formatAddress(tx.tx.account);
                    to = formatAddress(tx.tx.eth_address);
                    token = tx.tx.token;
                    token = tokens[token].symbol ? tokens[token].symbol : defaultTokenSymbol(token);
                    amount =  `${formatToken(tx.tx.amount, token)} ${token}`;
                    fee = `${formatToken(tx.tx.fee, token)} ${token}`;
                }

                return {
                type,
                from,
                to,
                amount,
                fee
            }})
        },
    },
    computed: {
        isBusy: () => false,
        blockNumber() {
            return this.$route.params.blockNumber
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
            ]
        },
        rows() {
            return this.items.length
        },
        props() {
            return [
                { name: 'Block #',          value: `<b>${this.blockNumber}</b>`},
                { name: 'New root hash',    value: this.new_state_root, },
                // { name: 'Transactions',     value: client.TX_PER_BLOCK(), },
                { name: 'Status',           value: this.status, },
                { name: 'Commit tx hash',   value: `<a target="blanc" href="${this.blockchain_explorer_tx}/${this.commit_tx_hash}">${this.commit_tx_hash}</a>`, },
                { name: 'Committed at',     value: this.committed_at},
                { name: 'Verify tx hash',   value: `<a target="blanc" href="${this.blockchain_explorer_tx}/${this.verify_tx_hash}">${this.verify_tx_hash}</a>`, },
                { name: 'Verified at',      value: this.verified_at},
            ]
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
        }
    }
}
</script>
