<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Franklin Network (Rinkeby)</b-navbar-brand>
        <b-navbar-brand right>API server: {{apiServer}}</b-navbar-brand>
    </b-container>
    </b-navbar>
            <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
    <br>
    <b-container>
        <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
        <b-card bg-variant="light" >
            <h5>Franklin Block Explorer</h5>
            <b-input-group>
                <b-form-input></b-form-input>
                <b-input-group-append>
                <b-button variant="info">Search</b-button>
                </b-input-group-append>
            </b-input-group>
        </b-card>
        <br>
        <b-card>
        <div class="row hide-sm" style="color: grey">
            <div class="col-sm text-center">
            <i class="far fa-square"></i> <b>Last committed</b><br><span class="num">328</span>
            </div>
            <div class="col-sm text-center">
            <i class="far fa-check-square"></i> <b>Last verified</b><br><span class="num">328</span>
            </div>
            <div class="col-sm text-center">
            <i class="fas fa-list"></i> <b>Total transactions</b><br><span class="num">17230</span>
            </div>
            <div class="col-sm text-center">
            <i class="fas fa-archive"></i> <b>Tx per block</b><br><span class="num">256</span>
            </div>
            <div class="col-sm hide-lg text-center">
            <i class="fas fa-tachometer-alt"></i> <b>Max TPS</b><br><span class="num">102</span>
            </div>
        </div>
        </b-card>
        <br>
        <b-table id="my-table" hover outlined :items="items" @row-clicked="onRowClicked"></b-table>
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

.num {
    font-size: 2.5em;
}

@media (max-width: 720px) {
.hide-sm {
    display: none
}
}

@media (max-width: 992px) {
.hide-lg {
    display: none
}
}
</style>

<script>

import store from './store'

export default {
    name: 'home',
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/blocks/' + item.block_number)
        }
    },
    data() {
      return {
          breadcrumbs: [
          {
            text: 'Blocks',
            active: true
          },
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

tr {
    cursor: pointer;
}
</style>