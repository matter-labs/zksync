<template>
<b-table responsive id="my-table" hover outlined :items="transactions" @row-clicked="onRowClicked"></b-table>
</template>

<script>

export default {
    name: 'transaction-list',
    props: ['blockNumber', 'transactions'],
    methods: {
        onRowClicked(item) {
            //this.$parent.$router.push('/transactions/' + item.id)
        }
    },
}
</script>

<style>
</style>