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
            <router-link :to="`/transactions/${data.item['tx_hash']}`">
                {{ shortenHash(data.item['tx_hash']) }}
            </router-link>
        </template>
        <template v-slot:cell(type)="data">
            <span v-html="data.item['type']" />
        </template>
        <template v-slot:cell(from)="data">
            <router-link :to="data.item['from_explorer_link']">
               <span v-html="data.item['from']" />
            </router-link>
        </template>
        <template v-slot:cell(to)="data">
             <router-link :to="data.item['to_explorer_link']">
               <span v-html="data.item['to']" />
            </router-link>
        </template>
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

            const hiddenFields = [
                'fromAddr', 
                'toAddr', 
                'success', 
                'from_explorer_link', 
                'to_explorer_link'
            ];

            return this.transactions.length == 0 ? []
                 : Object.keys(this.transactions[0]).filter(k => ! hiddenFields.includes(k));
        },
    },
    components,
};
</script>

<style>
</style>
