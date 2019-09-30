<template>
<b-container>
    <b-row>
        <b-col xl="6" class="pr-3">
            <BalancesList class="mb-1" balanceListId="onchain" :balances="onchainBalances"></BalancesList>
            <DepositButtons
                componentId="offchain"
                :topBalances="onchainBalances" 
                :bottomBalances="franklinBalances"
                :depositFeeNeeded="false"
                :withdrawFeeNeeded="true"
                v-on:depositEvent="deposit"
                v-on:withdrawEvent="withdraw"
                ></DepositButtons>
            <FranklinBalancesList class="mt-1" 
                balanceListId="franklin" 
                :balances="franklinBalancesWithInfo"
                ></FranklinBalancesList>
        </b-col>
        <b-col xl="6" class="pl-3">
            <Transfer
                :balances="franklinBalances"
                v-on:buttonClicked="transfer"
                v-on:alert="displayAlert"
            ></Transfer>
        </b-col>
    </b-row>
</b-container>
</template>

<script>
import { GeneratorMultiplier } from '../GeneratorMultiplier.js';

import Alert from './Alert.vue'
import BalancesList from './BalancesList.vue'
import FranklinBalancesList from './FranklinBalancesList.vue'
import DepositButtons from './DepositButtons.vue'
import Transfer from './Transfer.vue'
import ProgressBar from './ProgressBar.vue'

const components = {
    Alert,
    BalancesList,
    FranklinBalancesList,
    DepositButtons,
    Transfer,
    ProgressBar,
};

import { sleep } from '../utils.js'

export default {
    name: 'wallet',
    props: ['info'],
    data: () => ({
        message: '',
        onchainBalances: [],
        contractBalances: [],
        franklinBalances: [],
        franklinBalancesWithInfo: [],
        verboseShowerId: 0,
    }),
    watch: {
        info: function() {
            for (let [key, val] of Object.entries(this.info)) {
                this[key] = val;
            }
        }
    },
    methods: {
        displayAlert(kwargs) {
            this.$emit('alert', kwargs);
        },
        async deposit(kwargs) {
            await this.verboseShower(window.walletDecorator.verboseDeposit(kwargs));
        },
        async withdraw(kwargs) {
            await this.verboseShower(window.walletDecorator.verboseWithdraw(kwargs));
        },
        async transfer(kwargs) {
            await this.verboseShower(window.walletDecorator.verboseTransfer(kwargs));
        },
        async verboseShower(generator) {
            this.store.pendingTransactionGenerators.push({
                id: `verbose_shower_${this.verboseShowerId++}`,
                generator: new GeneratorMultiplier(generator),
            });
        },
        async verboseFunctionShower(generator) {
            for await (const progress of generator) {
                if (progress.message.includes(`waiting for creating new block`)) {
                    this.$refs.progress_bar && this.$refs.progress_bar.startProgressBarHalfLife(10000);
                }
                if (progress.message.includes(`started proving block`)) {
                    this.$refs.progress_bar && this.$refs.progress_bar.startProgressBarHalfLife(10000);
                }
                if (progress.message.includes(`got proved!`)) {
                    this.$refs.progress_bar && this.$refs.progress_bar.cancelAnimation();
                }
                this.$emit('alert', {
                    message: progress.message,
                    variant: progress.error ? 'danger' : 'success',
                });
            }
        },
        async transferFranklin(kwargs) {
            console.log('transfer', kwargs);
            try {
                if ( ! window.walletDecorator) {
                    displayAlert({ message: `Wallet is ${window.walletDecorator}` });
                    return;
                }

                await window.walletDecorator.transfer(kwargs);
            } catch (e) {
                this.displayAlert({ message: `unknown error: ${e}` });
            }
        },
    },
    components
}
</script>
