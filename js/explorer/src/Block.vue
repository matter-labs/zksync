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
            this.transactions = txs.map( (tx, index) => ({
                number:     index+1,
                type:       tx.tx_type,
                from:       tx.from,
                to:         tx.to,
                amount:     this.formatFranklin(tx.amount) + ' ETH',
                nonce:      tx.nonce,
            }))
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
                // { name: 'Type',             value: this.transactions.first().tx_type, },
                { name: 'New root hash',    value: this.new_state_root, },
                // { name: 'Transactions',     value: client.TX_PER_BLOCK(), },
                { name: 'Status',           value: this.status, },
                { name: 'Commit tx hash',   value: `<a target="blanc" href="${this.etherscan}/tx/${this.commit_tx_hash}">${this.commit_tx_hash}</a>`, },
                { name: 'Verify tx hash',   value: `<a target="blanc" href="${this.etherscan}/tx/${this.verify_tx_hash}">${this.verify_tx_hash}</a>`, },
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
