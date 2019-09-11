<template>
    <div>
        <b-row>
            <b-col align-h="around">
                <b-button class="my-2 w-100" variant="outline-secondary" v-b-modal="`${componentId}_depositModal`">&#x21E9; Deposit</b-button>
            </b-col>
            <b-col align-h="around">
                <b-button class="my-2 w-100" variant="outline-primary" v-b-modal="`${componentId}_withdrawModal`">Withdraw &#x21E7;</b-button>
            </b-col>
        </b-row>
        <b-modal v-bind:id="`${componentId}_depositModal`" hide-header hide-footer>
            <DepositWithdrawModal 
                windowTitle="Deposit"
                buttonText="Deposit"
                v-bind:balances="topBalances"
                v-bind:feeNeeded="depositFeeNeeded"
                v-on:buttonClicked="emitDeposit"
            ></DepositWithdrawModal>
        </b-modal>
        <b-modal v-bind:id="`${componentId}_withdrawModal`" hide-header hide-footer>
            <DepositWithdrawModal 
                windowTitle="Withdraw"
                buttonText="Withdraw"
                v-bind:balances="bottomBalances"
                v-bind:feeNeeded="withdrawFeeNeeded"
                v-on:buttonClicked="emitWithdraw"
            ></DepositWithdrawModal>
        </b-modal>
    </div>
</template>

<script>
import DepositWithdrawModal from './DepositWithdrawModal.vue'

const components = {
    DepositWithdrawModal
}

export default {
    name: 'DepositButtons',
    props: [
        'componentId', 'topBalances', 'bottomBalances', 
        'depositFeeNeeded', 'withdrawFeeNeeded'
    ],
    data: () => ({
        
    }),
    methods: {
        emitDeposit(kwargs) {
            this.$emit('depositEvent', kwargs);
        },
        emitWithdraw(kwargs) {
            this.$emit('withdrawEvent', kwargs);
        },
    },
    components
}
</script>
