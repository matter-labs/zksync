<template>
<div>
    <b-table responsive id="my-table" hover outlined :items="transactions" @row-clicked="onRowClicked" :fields="fields" class="clickable">
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
export default {
    name: 'transaction-list',
    props: ['blockNumber', 'transactions'],
    data: () => ({
        currentPage: 1,
        rowsPerPage: 10,
    }),
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.tx_hash);
        },
    },
    computed: {
        fields() {
            return this.transactions.length == 0 ? []
                 : Object.keys(this.transactions[0]).filter(k => k != 'tx_hash');
        },
    },
};
</script>

<style>
</style>
