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
        <b-card >
            <h5>Transaction #{{id}}</h5>
        </b-card>
        <br>
    </b-container>
</div>
</template>

<script>

import store from './store'

export default {
    name: 'transaction',
    methods: {
        test() {
            console.log('test')
        }
    },
    computed: {
        breadcrumbs() {
            return [
                {
                    text: 'Blocks',
                    to: '/'
                },
                {
                    text: ''+this.id,
                    active: true
                },
            ]
        },
        rows() {
            return this.items.length
        }
    },
    data() {
      return {
        id: 123,
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