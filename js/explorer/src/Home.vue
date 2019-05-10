<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Franklin Network (Testnet)</b-navbar-brand>
        <b-navbar-brand right>API server: {{apiServer}}</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-breadcrumb :items="bc"></b-breadcrumb>
        <b-card bg-variant="light" >
            <h6>Franklin Block Explorer</h6>
            <b-input-group>
                <b-form-input></b-form-input>
                <b-input-group-append>
                <b-button variant="info">Search</b-button>
                </b-input-group-append>
            </b-input-group>
        </b-card>
        <br>
        <b-card>
        <div class="row" style="color: grey">
            <div class="col-sm text-center">
            <i class="fas fa-cube"></i> <b>Total blocks:</b> 328
            </div>
            <div class="col-sm text-center">
            <i class="far fa-check-square"></i> <b>Total transactions:</b> 17230
            </div>
            <div class="col-sm text-center">
            <i class="fas fa-tachometer-alt"></i> <b>Max TPS:</b> 102
            </div>
        </div>
        </b-card>
        <br>
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
          bc: [
          {
            text: 'Blocks',
            href: '#'
          },
          {
            text: 'Manage',
            href: '#',
          },
          {
            text: 'Library',
            active: true
          }
        ],
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