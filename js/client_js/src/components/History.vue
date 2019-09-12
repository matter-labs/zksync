<template>
    <b-container>
        <b-row v-for="tx in transactions" v-bind:key="tx.elem_id">
            <b-col> {{ tx.type }} </b-col>
            <b-col> {{ tx.success }} </b-col>
            <b-col> {{ tx.fail_reason }} </b-col>
            <b-col> {{ tx.to }} </b-col>
            <b-col> {{ tx.amount }} </b-col>
            <b-col> {{ tx.is_committed }} </b-col>
            <b-col> {{ tx.is_verified }} </b-col>
        </b-row>
    </b-container>    
</template>

<script>
export default {
    name: 'History',
    props: ['info'],
    data: () => ({
        transactions: []
    }),
    watch: {
        info: function() {
            this.transactions = this.info.transactions;
        }
    },
    methods: {

    }
}
</script>
