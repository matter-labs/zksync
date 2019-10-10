<template>
    <b-container>
        <!-- <b-pagination v-if="ready" v-model="currentPage" per-page="10" :total-rows="transactions.length" @change="onPageChanged"></b-pagination> -->
        <HistoryRow v-for="txx in transactions" :tx="txx" :key="txx.elem_id"></HistoryRow>
        <!-- <b-pagination v-if="ready" v-model="currentPage" :per-page="perPage" :total-rows="rows" @change="onPageChanged"></b-pagination> -->
    </b-container>
</template>

<script>
import timeConstants from '../timeConstants'

import HistoryRow from './HistoryRow.vue'

const components = {
    HistoryRow
};

export default {
    name: 'History',
    data: () => ({
        transactions: [],
        perPage: 3,
        currentPage: 1,

        intervalHandle: null,
    }),
    async created() {
        await this.load();
        this.intervalHandle = setInterval(() => this.load(), timeConstants.transactionsRefresh);
    },
    destroyed() {
        clearInterval(this.intervalHandle); 
    },
    methods: {
        async load() {
            this.transactions = await window.walletDecorator.transactionsAsNeeded();
        },
    },
    components,
}
</script>
