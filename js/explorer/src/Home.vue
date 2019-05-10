<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Franklin Block Explorer</b-navbar-brand>
        <b-navbar-brand right>API server: {{apiServer}}</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <h5>Blocks</h5>
        <b-table id="my-table" hover outlined :items="items" @row-clicked="test"></b-table>
        <b-pagination
            v-model="currentPage"
            :total-rows="rows"
            :per-page="perPage"
            aria-controls="my-table"
        ></b-pagination>
    </b-container>
</div>
</template>

<style>
td {
    cursor: pointer;
}
</style>

<script>

import store from './store'

export default {
    name: 'home',
    methods: {
        test() {
            console.log('test')
        }
    },
    data() {
      return {
        perPage: 3,
        currentPage: 1,
        items: [
          { block_number: 1, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 2, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 3, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 4, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 5, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 6, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 7, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 8, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', },
          { block_number: 9, type: 'Transfer', transactions: 10, commit_hash: '0x070f6...a62e16f7d', verify_hash: '0x070f6...a62e16f7d', }
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

h1, h2, h3, h4 {
    font-weight: bold;
}

body {
    font-size: 0.9rem;
}

.btn {
    font-size: 0.8rem;
}
</style>