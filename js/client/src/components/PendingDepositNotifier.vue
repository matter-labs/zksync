<template>
    <b-container>
        <div class="py-5">
            <b-row>
                <b-col class="px-0 w-100 mb-2" style="text-align: center">
                    <h3>You have deposit transactions not yet completed.</h3>
                </b-col>
            </b-row>
            <b-row v-for="dep in pendingDeposits" :key="dep.elem_id">
                <CompleteDepositButton
                    class="w-100"
                    :dep="dep"
                    v-on:completionSuccess="completionSuccess"
                    ></CompleteDepositButton>
            </b-row>
        </div>
    </b-container>
</template>

<script>
import CompleteDepositButton from './CompleteDepositButton.vue';

const components = {
    CompleteDepositButton,
};

export default {
    name: 'PendingDepositNotifier',
    props: ['pendingDeposits'],
    methods: {
        completionSuccess() {

        },
        completeDeposit(deposit) {
            window.walletDecorator.completeDeposit(deposit.token, deposit.allowance);
        },
    },
    components,
}
</script>
