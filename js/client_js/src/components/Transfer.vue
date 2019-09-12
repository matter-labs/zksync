<template>
    <b-container>
        Address (for testing <code>0x241d9d2eebabfe5c07ea537f9c95ba3dd7fe87074ade45720eca8e</code>)
        <b-form-input type="text" v-model="address"></b-form-input>
        Token:
        <b-form-select v-model="token" class="mb-3">
            <option v-for="balance in balances" v-bind:key="balance.tokenName">{{ balance.tokenName }}</option>
        </b-form-select>
        Amount:
        <b-form-input type="number" v-model="amount"></b-form-input>
        Fee:
        <b-form-input type="number" v-model="fee"></b-form-input>
        <b-button class="mt-2" variant="primary" @click='buttonClicked'> Transfer </b-button>
    </b-container>
</template>

<script>
export default {
    name: 'Transfer',
    props: ['balances'],
    data: () => ({
        address: null,
        token: null,
        amount: null,
        fee: null,
    }),
    methods: {
        buttonClicked() {
            this.$emit('buttonClicked', {
                address: this.address,
                token: this.token,
                amount: this.amount,
                fee: this.fee
            });
        }
    }
}
</script>
