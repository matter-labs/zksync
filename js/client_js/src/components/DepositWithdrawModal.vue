<template>
    <div>
        Token:
        <!-- <b-form-select autocomplete="off" v-model="token" class="mb-2">
            <option v-for="balance in balances" :key="balance.tokenName">{{ balance.tokenName }}</option>
        </b-form-select> -->
        <TokenSelector 
            class="mb-2"
            :tokens="balances.map(b => b.tokenName)"
            :selected.sync="token">
        </TokenSelector>
        Amount <span v-if="maxAmountVisible">(<span v-if="token == 'ETH'">in ETH coins, </span>max {{ displayableBalancesDict[token] }} {{ token }})</span>:
        <b-form-input autocomplete="off" v-model="amount" class="mb-2"></b-form-input>
        <div v-if="feeNeeded">
            Fee:
            <!-- <b-form-input autocomplete="off" type="number" v-model="fee"></b-form-input> -->
            <FeeSelector 
                class="mb-2"
                :fees="fees"
                :selected.sync="fee">
            </FeeSelector>
        </div>
        <p v-if="alertVisible"> {{ alertText }} </p>
        <b-button class="w-50 mt-3" variant="primary" @click='buttonClicked'> {{ buttonText }} </b-button>
    </div>
</template>

<script>
import { bigNumberify, parseEther, formatUnits } from 'ethers/utils'
import { ethers } from 'ethers'
import { getDisplayableBalanceDict } from '../utils'

import TokenSelector from './TokenSelector.vue'
import FeeSelector from './FeeSelector.vue'

const components = {
    TokenSelector,
    FeeSelector,
};

export default {
    name: 'DepositWithdrawModal',
    props: [
        'buttonText',
        'balances',
        'feeNeeded',
    ],
    data: () => ({
        token: null,
        amount: null,
        fee: null,

        fees: [1, 10, 100], // TODO

        maxAmountVisible: false,
        balancesDict: {},
        displayableBalancesDict: {},
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
            this.displayableBalancesDict = getDisplayableBalanceDict(this.balancesDict);
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

            try {
                var amount = this.token == 'ETH'
                ? ethers.utils.parseEther(this.amount)
                : bigNumberify(this.amount);
            } catch (e) {
                this.localDisplayAlert(`Please input valid amount value`);
            }

            if (amount.gt(bigNumberify(this.balancesDict[this.token]))) {
                this.localDisplayAlert(`It's too much, man!`);
                return;
            }

            if (this.feeNeeded) {
                if (!this.fee) {
                    this.localDisplayAlert(`Select fee, please`);
                    return;
                }

                try {
                    var fee = this.token == 'ETH'
                    ? parseEther(this.fee)
                    : bigNumberify(this.fee);
                } catch (e) {
                    this.localDisplayAlert(`Please input valid fee value`);
                    return;
                }
                
                if (amount.add(fee).gt(bigNumberify(this.balancesDict[this.token]))) {
                    this.localDisplayAlert(`It's too much, man!`);
                    return;
                }
            }

            this.$emit('buttonClicked', {
                token: this.token,
                amount: amount,
                fee: fee,
            });
        }
    },
    components,
}
</script>
