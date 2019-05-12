<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Franklin Network (Rinkeby)</b-navbar-brand>
        <b-navbar-brand right>API server: {{apiServer}}</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <h5>Block data</h5>
        <b-card no-body>
            <b-table id="my-table" thead-class="hidden_header" :items="props" :busy="isBusy">
                <span slot="value" slot-scope="data" v-html="data.value"></span>
                <div slot="table-busy" class="text-center text-danger my-2">
                    <b-spinner class="align-middle"></b-spinner>
                    <strong>Loading...</strong>
                </div>
            </b-table>
        </b-card>
        <br>
        <h5>Transactions in this block</h5>
        <transaction-list :transactions="items"></transaction-list>
        <!--<b-table id="my-table" hover outlined :items="items" @row-clicked="onRowClicked"></b-table>-->
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
            if (block) {
                this.new_state_root  = block.new_state_root
                this.commit_tx_hash  = block.commit_tx_hash || ''
                this.verify_tx_hash  = block.verify_tx_hash || ''
                this.committed_at    = block.committed_at
                this.verified_at     = block.verified_at
                this.status          = block.verified_at ? 'Verified' : 'Committed'
            }
        },
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.id)
        }
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
                { name: 'Transactions',     value: client.TX_PER_BLOCK, },
                { name: 'Status',           value: this.status, },
                { name: 'Commit tx hash',   value: `<a target="blanc" href="https://rinkeby.etherscan.io/tx/${this.commit_tx_hash}">${this.commit_tx_hash}</a>`, },
                { name: 'Verify tx hash',   value: `<a target="blanc" href="https://rinkeby.etherscan.io/tx/${this.verify_tx_hash}">${this.verify_tx_hash}</a>`, },
            ]
        }
    },
    data() {
        return {
            new_state_root: null,
            commit_tx_hash: '',
            verify_tx_hash: '',
            committed_at:   null,
            verified_at:    null,
            status:         null,

            items: [
            { id: 1, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 2, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 3, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 4, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 5, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 6, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 7, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 8, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
            { id: 9, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, }
            ]
        }
    }
}
</script>

<style>
tr {
    cursor: pointer;
}
</style>