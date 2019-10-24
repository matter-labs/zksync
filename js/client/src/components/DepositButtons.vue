<template>
    <div>
        <b-row>
            <b-col align-h="around">
                <b-button 
                    class="my-2 w-100" 
                    variant="outline-primary" 
                    v-b-modal="`${componentId}_depositModal`"
                    :disabled="!!depositButtonDisabledReason"
                    :title="depositButtonDisabledReason"
                >&#x21E9; Deposit</b-button>
            </b-col>
            <b-col align-h="around">
                <b-button 
                    class="my-2 w-100" 
                    variant="outline-primary" 
                    v-b-modal="`${componentId}_withdrawModal`"
                    :disabled="!!withdrawButtonDisabledReason"
                    :title="withdrawButtonDisabledReason"
                >Withdraw &#x21E7;</b-button>
            </b-col>
        </b-row>
        <b-modal title="Deposit" :id="`${componentId}_depositModal`" hide-footer>
            <DepositWithdrawModal 
                buttonText="Deposit"
                :balances="topBalances"
                :feeNeeded="depositFeeNeeded"
                v-on:buttonClicked="emitDeposit"
            ></DepositWithdrawModal>
        </b-modal>
        <b-modal title="Withdraw" :id="`${componentId}_withdrawModal`" hide-footer>
            <DepositWithdrawModal 
                buttonText="Withdraw"
                :balances="bottomBalances"
                :feeNeeded="withdrawFeeNeeded"
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
        depositButtonDisabledBool: true,
        withdrawButtonDisabledBool: true,
    }),
    watch: {
        topBalances: function() {
            this.depositButtonDisabledBool = false;
        },
        bottomBalances: function() {
            this.withdrawButtonDisabledBool = false;
        }
    },
    computed: {
        depositButtonDisabledReason() {
            return this.depositButtonDisabledBool        ? "Balances not loaded yet."
                 : this.topBalances.length == 0          ? "You don't have any tokens yet."
                 : null;
        },
        withdrawButtonDisabledReason() {
            return this.withdrawButtonDisabledBool       ? "Balances not loaded yet."
                 : this.bottomBalances.length == 0       ? "You don't have any tokens yet."
                 : null;
        },
    },
    methods: {
        emitDeposit(options) {
            this.$bvModal.hide(`${this.componentId}_depositModal`);
            this.$emit('depositEvent', options);
        },
        emitWithdraw(options) {
            this.$bvModal.hide(`${this.componentId}_withdrawModal`);
            this.$emit('withdrawEvent', options);
        },
    },
    components
}
</script>
