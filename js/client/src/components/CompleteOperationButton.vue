<template>
    <div>
        <b-button 
            v-if="completionStatus == 'not clicked'"
            variant="primary" 
            class="w-100"
            @click="completeOperation"
            >Complete {{ op.operation }} {{ op.token.symbol }} {{ op.amountRenderable }}</b-button>
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
    methods: {
        async completeOperation() {
            this.completionStatus = 'Sending operation...';
            try {
                if (this.op.operation == 'Deposit') {
                    await window.walletDecorator.completeDeposit(this.op.token, this.op.amount);
                } else {
                    await window.walletDecorator.completeWithdraw(this.op.token, this.op.amount, this.op.hash);
                }
                this.completionStatus = 'Success, waiting for transaction to complete...';
                this.$emit('completionSuccess', { uniq_id: this.op.uniq_id });
            } catch (e) {
                console.log('error in CompleteOperationButton:', e);
                this.completionStatus = 'Something went wrong..';
                setTimeout(() => {
                    this.completionStatus = 'not clicked';
                }, 2000);
            }
        },
    },
}
</script>
