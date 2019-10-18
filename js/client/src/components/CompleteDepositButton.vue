<template>
    <div>
        <b-button 
            v-if="completionStatus == 'not clicked'"
            variant="primary" 
            class="w-100"
            @click="completeDeposit"
            >Complete deposit {{ dep.token.symbol }} {{ dep.allowanceRenderable }}</b-button>
        <img v-else-if="completionStatus == 'loading'" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
        <span v-else>{{ completionStatus }}</span>
    </div>
</template>

<script>
export default {
    name: 'CompleteDepositButton',
    props: ['dep'],
    data: () => ({
        completionStatus: 'not clicked',
    }),
    methods: {
        async completeDeposit() {
            this.completionStatus = 'loading';
            try {
                await window.walletDecorator.completeDeposit(this.dep.token, this.dep.allowance);
                this.completionStatus = 'Success, waiting for transaction to complete...';
                this.$emit('completionSuccess');
            } catch (e) {
                console.log('error in CompleteDepositButton:', e);
                this.completionStatus = 'Something went wrong..';
                setTimeout(() => {
                    this.completionStatus = 'not clicked';
                }, 2000);
            }
        },
    },
}
</script>
