<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Matter Testnet</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <h5>Transaction data</h5>
        <b-card no-body>
            <b-table responsive id="my-table" thead-class="hidden_header" :items="props" fixed>
                <span slot="value" slot-scope="data" v-html="data.value"></span>
            </b-table>
        </b-card>
        <br>
    </b-container>
</div>
</template>

<script>

import store from './store'

export default {
    name: 'transaction',
    computed: {
        id() {
            return this.$route.params.id
        },
        breadcrumbs() {
            return [
                {
                    text: 'All blocks',
                    to: '/'
                },
                {
                    text: 'Block ' + 123,
                    to: '/blocks/123'
                },                
                {
                    text: 'Transaction '+this.id,
                    active: true
                },
            ]
        },
        props() {
            return [
                { name: 'Id', value: `<b>${this.id}</b>`, empty1: '', empty2: ''},
                { name: 'Type', value: 'Unknown', },
                { name: 'From', value: '', },
                { name: 'To', value: '256', },
                { name: 'Amount', value: '1200', },
                { name: 'Nonce', value: '23', },
            ]
        }
    },
    data() {
      return {
        items: [
          { tx_id: 1, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 2, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 3, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 4, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 5, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 6, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 7, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 8, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
          { tx_id: 9, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, }
        ]
      }
    }
}
</script>

<style>
</style>