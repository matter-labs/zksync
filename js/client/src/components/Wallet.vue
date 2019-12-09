<template>
<b-container>
    <b-row>
        <b-card class="w-100 mx-3 mb-3" no-title>
            <b-row>
                <b-col class="col-auto">
                    <span style="line-height: 2em; font-size: 1.2em;">ZK Sync address:</span>
                </b-col>
                <b-col>
                    <CopyableAddress id="franklinAddressFormInput" :address="franklinAddress" />
                </b-col>
            </b-row>
        </b-card>
    </b-row>
    <b-row>
        <b-col xl="6" class="pr-3">
            <BalancesList 
                class="mb-1" 
                balanceListId="onchain" 
                :balances="onchainBalances"
                :pendingOps="pendingOps"
                v-on:withdrawOnchainEvent="withdrawOnchain"
                ></BalancesList>
            <DepositButtons
                componentId="offchain"
                :topBalances="onchainBalances" 
                :bottomBalances="franklinBalances"
                :depositFeeNeeded="false"
                :withdrawFeeNeeded="true"
                v-on:depositEvent="deposit"
                v-on:withdrawEvent="withdrawOffchain"
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
import { GeneratorMultiplierMinTime } from '../GeneratorMultiplier.js';

import Alert from './Alert.vue'
import BalancesList from './BalancesList.vue'
import FranklinBalancesList from './FranklinBalancesList.vue'
import DepositButtons from './DepositButtons.vue'
import Transfer from './Transfer.vue'
import ProgressBar from './ProgressBar.vue'
import CopyableAddress from './CopyableAddress.vue'

const components = {
    Alert,
    BalancesList,
    FranklinBalancesList,
    DepositButtons,
    Transfer,
    ProgressBar,
    CopyableAddress,
};

import { sleep } from '../utils.js'

export default {
    name: 'wallet',
    props: ['info'],
    data: () => ({
        message: '',
        onchainBalances: null,
        franklinBalances: null,
        franklinBalancesWithInfo: null,
        pendingOps: null,
    }),
    created() {
        this.updateInfo();
    },
    watch: {
        info() {
            this.updateInfo();
        }
    },
    methods: {
        updateInfo() {
            Object.assign(this, this.info);
        },
        displayAlert(options) {
            this.$emit('alert', options);
        },
        async deposit(options) {
            await this.showVerboseOperation(window.walletDecorator.verboseDeposit(options));
        },
        async withdrawOffchain(options) {
            await this.showVerboseOperation(window.walletDecorator.verboseWithdrawOffchain(options));
        },
        async withdrawOnchain(options) {
            await this.showVerboseOperation(window.walletDecorator.verboseWithdrawOnchain(options));
        },
        async transfer(options) {
            await this.showVerboseOperation(window.walletDecorator.verboseTransfer(options));
        },
        async showVerboseOperation(generator) {
            this.store.pendingTransactionGenerators.push({
                id: `verbose_verboseOp_${this.store.verboseOperationId++}`,
                generator: {
                    gencopy: () => generator,
                },
            });
        },
    },
    components
}
</script>
