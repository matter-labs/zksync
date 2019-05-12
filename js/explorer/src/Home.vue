<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Matter Network</b-navbar-brand>
        <b-navbar-toggle target="nav-collapse"></b-navbar-toggle>
        <b-collapse id="nav-collapse" is-nav>
        <b-navbar-nav>
            <b-nav-item href="#">Blocks</b-nav-item>
            <b-nav-item href="#">Transactions</b-nav-item>
            <b-nav-item href="#">MatterMask</b-nav-item>
        </b-navbar-nav>
        <b-navbar-nav class="ml-auto">
            <b-nav-item-dropdown text="Rinkeby" right>
                <b-dropdown-item href="#">Mainnet</b-dropdown-item>
                <b-dropdown-item href="#">Rinkeby</b-dropdown-item>
            </b-nav-item-dropdown>
        </b-navbar-nav>
        </b-collapse>
        <!--<b-navbar-brand right>API server: {{apiServer}}</b-navbar-brand>-->
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-card bg-variant="light" >
            <h5>Franklin Block Explorer</h5>
            <b-form @submit.stop.prevent="search">
            <b-input-group>
                <b-form-input placeholder="block number, root hash, tx hash or eth address"></b-form-input>
                <b-input-group-append>
                <b-button @click="search" variant="info" :disabled="searching">
                    <b-spinner v-if="searching" small></b-spinner>
                    <span>Search</span>
                </b-button>
                </b-input-group-append>
                <b-form-invalid-feedback v-if="notFound" :state="false">
                    Nothing found for xxx.
                </b-form-invalid-feedback>
            </b-input-group>
            </b-form>
        </b-card>
        <br>
        <b-card>
        <div class="row hide-sm" style="color: grey">
            <div class="col-sm text-center">
            <i class="far fa-square"></i> <b>Blocks committed</b><br><span class="num">{{lastCommitted}}</span>
            </div>
            <div class="col-sm text-center">
            <i class="far fa-check-square"></i> <b>Blocks verified</b><br><span class="num">{{lastVerified}}</span>
            </div>
            <div class="col-sm text-center">
            <i class="fas fa-list"></i> <b>Total transactions</b><br><span class="num">{{totalTransactions}}</span>
            </div>
            <div class="col-sm text-center">
            <i class="fas fa-archive"></i> <b>Tx per block</b><br><span class="num">{{txPerBlock}}</span>
            </div>
        </div>
        </b-card>
        <br>

        <!--
        <div class="table-container">
        <div class="overlay text-center" v-if="loading">
            <br><br><br>
            <b-spinner variant="primary"></b-spinner>
        </div>
        <b-table id="table" hover outlined :items="items" @row-clicked="onRowClicked" :busy="loading"></b-table>
        </div>
        -->

        <b-pagination v-model="currentPage" :per-page="perPage" :total-rows="lastCommitted" @change="onPageChanged"></b-pagination>
        <b-table id="table" hover outlined :items="items" @row-clicked="onRowClicked" :busy="loading"></b-table>
        <b-pagination v-model="currentPage" :per-page="perPage" :total-rows="lastCommitted" @change="onPageChanged"></b-pagination>

    </b-container>
</div>
</template>

<style>

.table-container {
  position: relative;
}

.overlay {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 100%;
}

td {
    cursor: pointer;
}

.num {
    font-size: 3em;
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

<script>

import store from './store'
import client from './client'

export default {
    name: 'home',
    created() {
        this.update()
    },
    methods: {
        async search() {

            this.update()

            // this.searching = true
            // this.notFound = false
            // await new Promise(resolve => setTimeout(resolve, 200))
            // this.searching = false
            // this.notFound = true
            // await new Promise(resolve => setTimeout(resolve, 3600))
            // this.notFound = false
        },
        onRowClicked(item) {
            this.$parent.$router.push('/blocks/' + item.block_number)
        },
        async onPageChanged(page) {
            //this.loading = true
            //await new Promise(resolve => setTimeout(resolve, 600))
            //this.loading = false
            console.log(page)
            this.currentPage = page
            this.updateBlocks()
        },
        async updateBlocks() {
            let max = this.lastCommitted - (client.PAGE_SIZE * (this.currentPage-1))
            console.log('u', this.lastCommitted, client.PAGE_SIZE, (this.currentPage-1), max)

            let blocks = await client.loadBlocks(max)
            if (blocks) {
                this.blocks = blocks.map( b => ({
                    block_number:   b.block_number,
                    status:         b.verified_at ? 'Verified' : 'Committed',
                    new_state_root: b.new_state_root.slice(0, 16) + '...' + b.new_state_root.slice(50, 66),
                    committed_at:   b.committed_at,
                    verified_at:    b.verified_at,
                }))
                console.log(blocks)
            }
            this.loading = false
        },
        async update() {
            this.loading = false
            const status = await client.status()
            let newBlocks = false
            if (status) {
                console.log(status)
                newBlocks = this.lastCommitted !== status.last_committed || this.lastVerified !== status.last_verified
                this.lastCommitted = status.last_committed
                this.lastVerified = status.last_verified
                this.totalTransactions = status.total_transactions

            }
            if (newBlocks) {
                this.updateBlocks()
            } else {
                this.loading = false
            }
        }
    },
    computed: {
        items() {
            return this.blocks
        },
        pages() {

        }
    },
    data() {
      return {
        lastCommitted:      0,
        lastVerified:       0,
        totalTransactions:  0,
        
        txPerBlock:         client.TX_PER_BLOCK,
        blocks:             [],

        breadcrumbs: [
          {
            text: 'Blocks',
            active: true
          },
        ],
        loading:        true,
        searching:      false,
        notFound:       false,

        perPage:        20,
        rows:           2000,
        currentPage:    1,
        // items: [
        //   { block_number: 1, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 2, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 3, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 4, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 5, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 6, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 7, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 8, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' },
        //   { block_number: 9, status: 'Verified', type: 'Transfer', transactions: 10, new_root_hash: '0x070f6...a62e16f7d' }
        // ]
      }
    },
}
</script>
