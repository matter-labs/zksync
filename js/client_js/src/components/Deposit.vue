<template>
    <b-card title="Deposit">
        Token:
        <b-form-input type="text" v-model="token"></b-form-input>
        Amount:
        <b-form-input type="number" v-model="amount"></b-form-input>
        <b-button href="#" variant="primary" @click='deposit'>Deposit</b-button>
    </b-card>
</template>

<script>
export default {
    name: 'Deposit',
    data: () => ({
        'token': null,
        'amount': null
    }),
    methods: {
        async deposit() {
            try {
                if ( ! window.wallet) {
                    this.$emit('alert', `Wallet is ${window.wallet}`);
                    return;
                }
    
                await window.wallet.deposit();
    
                console.log('wallet', window.wallet);
                console.log('token', this.token);
                console.log('amount', this.amount);
    
                this.$emit('alert', `deposit succeeded or something`);
            } catch (e) {
                this.$emit('alert', `unknown error: ${e.msg}`);
            }
        }
    }
}
</script>
