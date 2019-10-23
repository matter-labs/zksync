<template>
    <div class="operationButtonContainer ml-3">
        <a href="#" 
            v-if="completionStatus == 'not clicked'"
            variant="primary" 
            class="w-100"
            @click="completeOperation"
            >Complete {{ op.operation }} {{ op.token.symbol }} {{ op.amountRenderable }}</a>
        <img v-else-if="completionStatus == 'loading'" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
        <span v-else>{{ completionStatus }}</span>
    </div>
</template>

<script>
export default {
    name: 'CompleteOperationButton',
    props: ['op'],
    data: () => ({
        completionStatus: 'not clicked',
    }),
    created() {
        let status = this.store.withdrawCompletionStatusDict[this.op.hash];
        if (status != undefined) {
            this.completionStatus = status;
        }
    },
    methods: {
        async completeOperation() {
            this.completionStatus = 'Sending operation...';
            this.store.withdrawCompletionStatusDict[this.op.hash] = 'Sending operation...';
            try {
                await window.walletDecorator.completeWithdraw(this.op.token, this.op.amount, this.op.hash);
                this.completionStatus = 'Success, waiting for transaction to complete...';
                this.store.withdrawCompletionStatusDict[this.op.hash] = 'Success, waiting for transaction to complete...';
                this.$emit('completionSuccess', { uniq_id: this.op.uniq_id });
            } catch (e) {
                console.log('error in CompleteOperationButton:', e);
                this.completionStatus = 'Something went wrong..';
                this.store.withdrawCompletionStatusDict[this.op.hash] = 'Something went wrong..';
                setTimeout(() => {
                    delete this.store.withdrawCompletionStatusDict[this.op.hash];
                    this.completionStatus = 'not clicked';
                }, 2000);
            }
        },
    },
}
</script>

<style scoped>
.operationButtonContainer {
    display: inline-block; 
    vertical-align: middle;
    opacity: 0.7;
}
.operationButtonContainer:hover {
    opacity: 1;
}
</style>
