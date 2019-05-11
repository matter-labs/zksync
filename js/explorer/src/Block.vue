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

export default {
    name: 'block',
    components: {
        'transaction-list':  TransactionList
    },
    methods: {
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
                { name: 'Block #', value: `<b>${this.blockNumber}</b>`},
                { name: 'New root hash', value: '0x0d6724d559efd3a85e2aa78e053a73e612dcedb19b09ea1be67e8393a0278bda', },
                { name: 'Transactions', value: '256', },
                { name: 'Status', value: '<b-badge>Verified</b-badge>', },
                { name: 'Commit tx hash', value: '<a href="#">0x0d6724d559efd3a85e2aa78e053a73e612dcedb19b09ea1be67e8393a0278bda</a>', },
                { name: 'Verify tx hash', value: '<a href="#">0x0d6724d559efd3a85e2aa78e053a73e612dcedb19b09ea1be67e8393a0278bda</a>', },
            ]
        }
    },
    data() {
      return {
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