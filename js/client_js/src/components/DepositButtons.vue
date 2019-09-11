<template>
    <div>
        <b-row>
            <b-col>
                <b-button v-b-modal="`${componentId}_depositModal`">Deposit</b-button>
            </b-col>
            <b-col>
                <b-button v-b-modal="`${componentId}_withdrawModal`">Withdraw</b-button>
            </b-col>
        </b-row>
        <b-modal v-bind:id="`${componentId}_depositModal`" hide-header hide-footer>
            <DepositWithdraw 
                windowTitle="Deposit"
                buttonText="Deposit"
                v-bind:balances="topBalances"
                v-on:buttonClicked="emitDeposit"
            ></DepositWithdraw>
        </b-modal>
        <b-modal v-bind:id="`${componentId}_withdrawModal`" hide-header hide-footer>
            <DepositWithdraw 
                windowTitle="Withdraw"
                buttonText="Withdraw"
                v-bind:balances="bottomBalances"
                v-on:buttonClicked="emitWithdraw"
            ></DepositWithdraw>
        </b-modal>
    </div>
</template>

<script>
import DepositWithdraw from './DepositWithdraw.vue'

const components = {
    DepositWithdraw
}

export default {
    name: 'DepositButtons',
    props: ['componentId', 'topBalances', 'bottomBalances'],
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
