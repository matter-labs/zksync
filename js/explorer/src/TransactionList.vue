<template>
<b-table id="my-table" hover outlined :items="transactions" @row-clicked="onRowClicked"></b-table>
</template>

<script>

import store from './store'

export default {
    name: 'transaction-list',
    props: ['blockNumber', 'transactions'],
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.id)
        }
    },
    data() {
      return {
        // items: [
        //   { tx_id: 1, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 2, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 3, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 4, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 5, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 6, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 7, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 8, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, },
        //   { tx_id: 9, type: 'Transfer', from: 2, to: 4, amount: 123, nonce: 87, }
        // ]
      }
    },
}
</script>

<style>
</style>