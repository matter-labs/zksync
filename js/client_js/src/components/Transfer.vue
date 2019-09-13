<template>
    <b-container>
        Address (for testing <code>0x241d9d2eebabfe5c07ea537f9c95ba3dd7fe87074ade45720eca8e</code>)
        <b-form-input type="text" v-model="address"></b-form-input>
        Token:
        <b-form-select v-model="token" class="mb-3">
            <option v-for="balance in balances" v-bind:key="balance.tokenName">{{ balance.tokenName }}</option>
        </b-form-select>
        Amount <span v-if="maxAmountVisible">(no more than {{ token }} {{ balancesDict[token] }}</span>:
        <b-form-input type="number" v-model="amount"></b-form-input>
        Fee:
        <b-form-input type="number" v-model="fee"></b-form-input>
        <b-button class="mt-2" variant="primary" @click='buttonClicked'> Transfer </b-button>
    </b-container>
</template>

<script>
import { bigNumberify } from 'ethers/utils'

export default {
    name: 'Transfer',
    props: ['balances'],
    data: () => ({
        address: null,
        token: null,
        amount: null,
        fee: null,

        maxAmountVisible: false,
        balancesDict: {},
    }),
    watch: {
        balances: function() {
            this.balancesDict = this.balances
                .reduce((acc, bal) => {
                    acc[bal.tokenName] = bal.amount;
                    return acc;
                }, {});
        },
        token: function() {
            this.maxAmountVisible = true;
        }
    },
    methods: {
        localDisplayAlert(message) {
            this.$emit('alert', { message });
        },
        buttonClicked() {
            const addressLength = '0x241d9d2eebabfe5c07ea537f9c95ba3dd7fe87074ade45720eca8e'.length;
            if (!this.address) {
                this.localDisplayAlert(`Please select address.`);
                return;
            }
            if (this.address.length != addressLength) {
                this.localDisplayAlert(`Franklin addresses are hex strings`
                    + `of length ${addressLength}. Are you sure this is a Franklin address?`);
                return;
            }
            if (this.address.startsWith('0x') === false) {
                this.localDisplayAlert(`Franklin addresses are hex strings starting with 0x`
                    + `Are you sure this is a Franklin address?`);
                return;
            }

            if (!this.token) {
                this.localDisplayAlert(`Please select token.`);
                return;
            }

            if (!this.amount) {
                this.localDisplayAlert(`Please select amount.`);
                return;
            }

            if (!this.fee) {
                this.localDisplayAlert(`Please select fee.`);
                return;
            }

            if (bigNumberify(this.amount).add(this.fee).gt(bigNumberify(this.balancesDict[this.token]))) {
                this.localDisplayAlert(`It's too much, man!`);
                return;
            }

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
