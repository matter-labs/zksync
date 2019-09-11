<template>
    <b-card v-bind:title="windowTitle">
        Token:
        <b-form-select v-model="token" class="mb-3">
            <option v-for="balance in balances" v-bind:key="balance.tokenName">{{ balance.tokenName }}</option>
        </b-form-select>
        Amount:
        <b-form-input type="number" v-model="amount"></b-form-input>
        <b-button href="#" variant="primary" @click='buttonClicked'> {{ buttonText }} </b-button>
    </b-card>
</template>

<script>
export default {
    name: 'DepositWithdraw',
    props: [
        'windowTitle',
        'buttonText',
        'balances'
    ],
    data: () => ({
        'token': null,
        'amount': null
    }),
    methods: {
        async buttonClicked() {
            this.$emit('buttonClicked', {
                token: this.token,
                amount: this.amount
            });
        }
    }
}
</script>
