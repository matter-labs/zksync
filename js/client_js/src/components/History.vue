<template>
    <b-container>
        <HistoryRow v-for="txx in transactions" :tx="txx" :key="txx.elem_id"></HistoryRow>
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
    watch: {
        info: function() {
            this.transactions = this.info.transactions;
        }
    },
    methods: {

    },
    components,
}
</script>
