<template>
    <b-card title="Main chain">
        <b-col>
            <img v-if="disabledReason == 'Not loaded yet.'" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
            <p class="mt-3" v-else-if="disabledReason == 'No tokens'">
                <b>Your Main chain balance is empty.</b>
            </p>
            <b-table v-else class="b-table-balances-width-hack" borderless small responsive :fields="fields" :items="displayableBalances">
                <template v-slot:cell(tokenName)="data">
                    <TokenNameButton :data="data"></TokenNameButton>
                </template>
                <template v-slot:cell(amount)="data">
                    <span style="vertical-align: middle;"> {{ data.item.amount }} </span>
                    <CompleteOperationButton
                        v-if="data.item.op && data.item.op.status != 'hidden'"
                        :op="data.item.op"
                        v-on:withdrawOnchainEvent="withdrawOnchainEvent"
                        ></CompleteOperationButton>
                </template>
            </b-table>
            <span v-if="disabledReason != 'Not loaded yet.'">
                You can get some 
                <a href="https://faucet.rinkeby.io/" target="_blank">ETH</a>
                or
                <a href="https://erc20faucet.com/" target="_blank">ERC20</a>
                tokens to play with.
            </span>
        </b-col>
    </b-card>
</template>

<script>
import { ethers } from 'ethers';
import { readableEther, getDisplayableBalanceList, sleep } from '../utils';

import TokenNameButton from './TokenNameButton.vue';
import CompleteOperationButton from './CompleteOperationButton.vue';

const components = {
    TokenNameButton,
    CompleteOperationButton,
};


export default {
    name: 'BalancesList',
    data: () => ({
        fields: [
            { key: 'tokenName', label: 'Token' }, 
            'amount'
        ],
        displayableBalances: [],
    }),
    props: [
        // balances are like [{ tokenName: 'eth', amount: '120' }]
        'balances',
        'balanceListId',
        'pendingOps',
    ],
    created() {
        this.updateBalanceList();
    },
    watch: {
        balances() {
            this.updateBalanceList();
        },
        pendingOps() {
            this.updateBalanceList();
        },
    },
    computed: {
        disabledReason() {
            return this.balances == null        ? "Not loaded yet."
                 : this.balances.length == 0    ? "No tokens"
                 : null;
        },
    },
    methods: {
        async withdrawOnchainEvent(options) {
            this.$emit('withdrawOnchainEvent', options);
        },
        updateBalanceList() {
            if (this.balances != null) {
                this.displayableBalances = getDisplayableBalanceList(this.balances);
            }

            if (this.pendingOps != null && this.pendingOps.length) {
                let pendingOpsIndex = this.pendingOps
                    .reduce((acc, op) => {
                        acc[op.token.address] = op;
                        return acc;
                    }, {});

                this.displayableBalances = this.displayableBalances
                    .map(bal => {
                        if (pendingOpsIndex[bal.address]) {
                            bal.op = pendingOpsIndex[bal.address];
                        }
                        return bal;
                    });
            }
        },
    },
    components,
}
</script>

<style scoped>
.tokenNameButton {
    display: inline-block;
    height: 2;
}
</style>
