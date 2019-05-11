<template>
<b-table id="my-table" hover outlined :items="items" @row-clicked="test"></b-table>
</template>

<script>

import store from './store'

export default {
    name: 'transaction-list',
    methods: {
    },
    data() {
      return {
        perPage: 3,
        currentPage: 1,
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
    },
    computed: {
      rows() {
        return this.items.length
      }
    }
}
</script>

<style>
</style>