<template>
<b-table responsive id="my-table" hover outlined :items="transactions" @row-clicked="onRowClicked" :fields="fields" class="clickable">
    <template v-slot:cell(type)="data"><span v-html="data.item['type']" /></template>
    <template v-slot:cell(from)="data"><span v-html="data.item['from']" /></template>
    <template v-slot:cell(to)="data"><span v-html="data.item['to']" /></template>
</b-table>
</template>

<script>

export default {
    name: 'transaction-list',
    props: ['blockNumber', 'transactions'],
    methods: {
        onRowClicked(item) {
            this.$parent.$router.push('/transactions/' + item.tx_hash)
        }
    },
    computed: {
        fields() {
            return this.transactions.length == 0 ? []
                 : Object.keys(this.transactions[0]).filter(k => k != 'tx_hash');
        }
    }
};
</script>

<style>
</style>
