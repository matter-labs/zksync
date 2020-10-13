<template>
<div>
    <b-table 
        responsive 
        id="my-table" 
        hover 
        outlined 
        :items="transactions" 
        :per-page="rowsPerPage" 
        :current-page="currentPage"
        :fields="fields" 
        class="nowrap"
    >
        <template v-slot:cell(tx_hash)="data">
            <a :href="`/transactions/${data.item['tx_hash']}`">
                {{ shortenHash(data.item['tx_hash']) }}
            </a>
        </template>
        <template v-slot:cell(type)="data"><span v-html="data.item['type']" /></template>
        <template v-slot:cell(from)="data"><span v-html="data.item['from']" /></template>
        <template v-slot:cell(to)="data"><span v-html="data.item['to']" /></template>
    </b-table>
    <b-pagination 
        v-if="transactions.length > rowsPerPage"
        class="mt-2 mb-2"
        v-model="currentPage" 
        :per-page="rowsPerPage" 
        :total-rows="transactions.length"
    ></b-pagination>
</div>
</template>

<script>
import CopyableAddress from './CopyableAddress.vue';
import { shortenHash } from './utils';

const components = {
    CopyableAddress,
};

export default {
    name: 'transaction-list',
    props: ['blockNumber', 'transactions'],
    data: () => ({
        currentPage: 1,
        rowsPerPage: 1000,
    }),
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.tx_hash);
        },
        shortenHash,
    },
    computed: {
        fields() {
            return this.transactions.length == 0 ? []
                 : Object.keys(this.transactions[0]).filter(k => ! ['fromAddr', 'toAddr', 'success'].includes(k));
        },
    },
    components,
};
</script>

<style>
</style>
