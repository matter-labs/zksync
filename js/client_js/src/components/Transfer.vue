<template>
    <b-card title="Transfer in Matter network" class="px-0">
        Address:
        <b-form-input autocomplete="off" type="text" v-model="address" class="mb-2"></b-form-input>
        <p>(for testing, use <code style="cursor: pointer" @click="address='0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e'">0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e</code>)</p>
        Choose token:
        <!-- <b-form-select v-model="token" class="mb-2">
            <option v-for="balance in balances" :key="balance.tokenName">{{ balance.tokenName }}</option>
        </b-form-select> -->
        <TokenSelector 
            class="mb-3"
            :tokens="tokensList"
            :selected.sync="token">
        </TokenSelector>
        Amount <span v-if="maxAmountVisible">(max {{ token }} {{ balancesDict[token] }})</span>:
        <b-form-input autocomplete="off" type="number" v-model="amount" class="mb-3"></b-form-input>
        Choose fee:
        <FeeSelector 
            class="mb-3"
            :fees="fees"
            :selected.sync="fee">
        </FeeSelector>
        <!-- <b-form-input autocomplete="off" type="number" class="mb-3" v-model="fee"></b-form-input> -->
        <b-button class="mt-2 w-50" variant="primary" @click='buttonClicked'> Transfer </b-button>
    </b-card>
</template>

<script>
import { bigNumberify } from 'ethers/utils'
import TokenSelector from './TokenSelector.vue'
import FeeSelector from './FeeSelector.vue'

const components = {
    TokenSelector,
    FeeSelector,
};

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
        tokensList: [],
        fees: [1, 10, 100], // TODO: these should be computed somehow idk
    }),
    watch: {
        balances: function() {
            this.balancesDict = this.balances
                .reduce((acc, bal) => {
                    acc[bal.tokenName] = bal.amount;
                    return acc;
                }, {});
            this.tokensList = this.balances.map(bal => bal.tokenName);
        },
        token: function() {
            this.maxAmountVisible = true;
        }
    },
    methods: {
        localDisplayAlert(message) {
            this.$emit('alert', { message, variant: 'warning' });
        },
        buttonClicked() {
            const addressLength = '0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e'.length;
            if (!this.address) {
                this.localDisplayAlert(`Please select address.`);
                return;
            }
            if (this.address.length != addressLength) {
                this.localDisplayAlert(`Matter Testnet addresses are hex strings`
                    + `of length ${addressLength}. Are you sure this is a Matter Testnet address?`);
                return;
            }
            if (this.address.startsWith('0x') === false) {
                this.localDisplayAlert(`Matter Testnet addresses are hex strings starting with 0x`
                    + `Are you sure this is a Matter Testnet address?`);
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
    },
    components,
}
</script>

<style scoped>
</style>
