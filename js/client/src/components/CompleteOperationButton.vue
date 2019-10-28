<template>
    <span class="operationButtonContainer ml-3">
        <a href="#" 
            v-if="completionStatus == 'not clicked'"
            variant="primary" 
            class="w-100"
            @click="completeOperation"
            >Complete {{ op.operation }} {{ op.token.symbol }} {{ op.amountRenderable }}</a>
        <img v-else-if="completionStatus == 'loading'" style="margin-right: 1.5em" src="../assets/loading.gif" width="50em">
        <span v-else>{{ completionStatus }}</span>
    </span>
</template>

<script>
export default {
    name: 'CompleteOperationButton',
    props: ['op'],
    data: () => ({
        completionStatus: null,
    }),
    created() {
        this.completionStatus = this.op.status || 'not clicked';
    },
    methods: {
        async completeOperation() {
            this.$emit('withdrawOnchainEvent', this.op);
        },
    },
}
</script>

<style scoped>
.operationButtonContainer {
    vertical-align: middle;
    opacity: 0.7;
}
.operationButtonContainer:hover {
    opacity: 1;
}
</style>
