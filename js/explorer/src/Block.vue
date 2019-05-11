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
            <h5>Block #{{blockNumber}}</h5>
        </b-card>
        <br>
        <h6>Block transactions</h6>
        <b-table id="my-table" hover outlined :items="items" @row-clicked="onRowClicked"></b-table>
    </b-container>
</div>
</template>

<script>

import store from './store'

export default {
    name: 'block',
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.id)
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
                    text: ''+this.blockNumber,
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
        blockNumber: 123,
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