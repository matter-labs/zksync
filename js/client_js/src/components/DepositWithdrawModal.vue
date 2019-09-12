<template>
    <b-card v-bind:title="windowTitle">
        Token:
        <b-form-select v-model="token" class="mb-3">
            <option v-for="balance in balances" v-bind:key="balance.tokenName">{{ balance.tokenName }}</option>
        </b-form-select>
        Amount <span v-if="maxAmountVisible">(no more than {{ token }} {{ balancesDict[token] }}</span>:
        <b-form-input type="number" v-model="amount"></b-form-input>
        <b-row v-if="feeNeeded">
            Fee:
            <b-form-input type="number" v-model="fee"></b-form-input>
        </b-row>
        <b-row v-if="alertVisible"> {{ alertText }} </b-row>
        <b-button class="mt-3" variant="primary" @click='buttonClicked'> {{ buttonText }} </b-button>
    </b-card>
</template>

<script>
import { bigNumberify } from 'ethers/utils'

export default {
    name: 'DepositWithdrawModal',
    props: [
        'windowTitle',
        'buttonText',
        'balances',
        'feeNeeded',
    ],
    data: () => ({
        token: null,
        amount: null,
        fee: null,

        maxAmountVisible: false,
        balancesDict: {},
        alertVisible: false,
        alertText: '',
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
        localDisplayAlert(msg) {
            this.alertVisible = true;
            this.alertText = msg;
        },
        async buttonClicked() {
            if (!this.token) {
                this.localDisplayAlert(`Select token, please`);
                return;
            }
            if (!this.amount) {
                this.localDisplayAlert(`Select amount, please`);
                return;
            }
            if (bigNumberify(this.amount).gt(bigNumberify(this.balancesDict[this.token]))) {
                this.localDisplayAlert(`It's too much, man!`);
                return;
            }

            if (this.feeNeeded) {
                if (!this.fee) {
                    this.localDisplayAlert(`Select fee, please`);
                    return;
                }
                if (bigNumberify(this.amount).add(this.fee).gt(bigNumberify(this.balancesDict[this.token]))) {
                    this.localDisplayAlert(`It's too much, man!`);
                    return;
                }
            }

            this.$emit('buttonClicked', {
                token: this.token,
                amount: this.amount,
                fee: this.fee,
            });
        }
    }
}
</script>
