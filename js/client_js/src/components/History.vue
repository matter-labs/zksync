<template>
    <b-container>
        <!-- <b-pagination v-if="ready" v-model="currentPage" per-page="10" :total-rows="transactions.length" @change="onPageChanged"></b-pagination> -->
        <HistoryRow v-for="txx in transactions" :tx="txx" :key="txx.elem_id"></HistoryRow>
        <!-- <b-pagination v-if="ready" v-model="currentPage" :per-page="perPage" :total-rows="rows" @change="onPageChanged"></b-pagination> -->
    </b-container>
</template>

<script>
import HistoryRow from './HistoryRow.vue'

const components = {
    HistoryRow
};

export default {
    name: 'History',
    props: ['info'],
    data: () => ({
        transactions: [],
        perPage: 3,
        currentPage: 1,
    }),
    created() {
        this.transactions = this.info.transactions;
    },
    watch: {
        info: function() {
            this.transactions = this.info.transactions;
        },
    },
    methods: {
        onPageChanged(...args) {
            console.log('onPageChanged', args);
        },
    },
    components,
}
</script>
