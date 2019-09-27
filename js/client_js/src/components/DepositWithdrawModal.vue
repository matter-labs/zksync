<template>
    <div>
        Token:
        <TokenSelector 
            class="mb-2"
            :tokens="balances.map(b => b.tokenName)"
            :selected.sync="token">
        </TokenSelector>
        Amount <span v-if="maxAmountVisible">(<span v-if="token == 'ETH'">in ETH coins, </span>max {{ displayableBalancesDict[token] }} {{ token }})</span>:
        <b-form-input autocomplete="off" v-model="amountSelected" class="mb-2"></b-form-input>
        <div v-if="feeNeeded">
            Fee:
            <FeeSelector 
                class="mb-2"
                :fees="fees"
                :selected.sync="feeButtonSelectedIndex">
            </FeeSelector>
        </div>
        <p v-if="alertVisible"> {{ alertText }} </p>
        <b-button class="w-50 mt-3" variant="primary" @click='buttonClicked'> {{ buttonText }} </b-button>
    </div>
</template>

<script>
import { bigNumberify, parseEther, formatUnits } from 'ethers/utils'
import { ethers } from 'ethers'
import { getDisplayableBalanceDict, feesFromAmount } from '../utils'

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

        amountSelected: null,
        feeButtonSelectedIndex: null,
        fees: ['Normal', 'Faster(1%)', 'Fastest(5%)'],

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
        getAmount() {
            try {
                return this.token == 'ETH'
                    ? parseEther(this.amountSelected)
                    : bigNumberify(this.amountSelected);
            } catch (e) {
                console.log('amount compute error: ', e);
                return null;
            }
        },
        getFee() {
            try {
                let amount = this.getAmount();
                console.log('amount', amount);
                console.log('this.feeButtonSelectedIndex', this.feeButtonSelectedIndex);
                return feesFromAmount(amount)[this.feeButtonSelectedIndex];
            } catch (e) {
                console.log(e);
                return null;
            }
        },
        async buttonClicked() {
            if (!this.token) {
                this.localDisplayAlert(`Please select token.`);
                return;
            }

            if (this.amountSelected == null) {
                this.localDisplayAlert(`Please select amount`);
                return;
            }

            let amount = this.getAmount();
            if (amount == null) {
                this.localDisplayAlert(`Please input valid amount value`);
                return;
            }

            if (this.feeNeeded) {
                if (this.feeButtonSelectedIndex == null) {
                    this.localDisplayAlert(`Please select fee`);
                    return;
                }
                
                var fee = this.getFee();
                if (fee == null) {
                    this.localDisplayAlert(`Problem with fee.`); // TODO:
                    return;
                }
    
                if (amount.add(fee).gt(bigNumberify(this.balancesDict[this.token]))) {
                    this.localDisplayAlert(`It's too much, man!`);
                    return;
                }
            } else {
                if (amount.gt(bigNumberify(this.balancesDict[this.token]))) {
                    this.localDisplayAlert(`It's too much, man!`);
                    return;
                }
            }

            // if (!this.token) {
            //     this.localDisplayAlert(`Select token, please`);
            //     return;
            // }
            // if (!this.amount) {
            //     this.localDisplayAlert(`Select amount, please`);
            //     return;
            // }

            // try {
            //     var amount = this.token == 'ETH'
            //     ? ethers.utils.parseEther(this.amount)
            //     : bigNumberify(this.amount);
            // } catch (e) {
            //     this.localDisplayAlert(`Please input valid amount value`);
            // }

            // if (amount.gt(bigNumberify(this.balancesDict[this.token]))) {
            //     this.localDisplayAlert(`It's too much, man!`);
            //     return;
            // }

            // if (this.feeNeeded) {
            //     if (!this.fee) {
            //         this.localDisplayAlert(`Select fee, please`);
            //         return;
            //     }

            //     try {
            //         var fee = this.token == 'ETH'
            //         ? parseEther(this.fee)
            //         : bigNumberify(this.fee);
            //     } catch (e) {
            //         this.localDisplayAlert(`Please input valid fee value`);
            //         return;
            //     }
                
            //     if (amount.add(fee).gt(bigNumberify(this.balancesDict[this.token]))) {
            //         this.localDisplayAlert(`It's too much, man!`);
            //         return;
            //     }
            // }

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
