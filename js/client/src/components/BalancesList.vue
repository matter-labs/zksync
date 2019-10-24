<template>
    <b-card title="Main chain">
        <b-col>
            <label for="ethereumAddressFormInput">Address</label> 
                (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+ethereumAddress"
                    target="blanc">block explorer</a>):
            <CopyableAddress id="ethereumAddressFormInput" :address="ethereumAddress"></CopyableAddress>
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
                        v-if="data.item.op"
                        :op="data.item.op"
                        v-on:completionSuccess="updatePendingOps"
                        ></CompleteOperationButton>
                </template>
            </b-table>
        </b-col>
    </b-card>
</template>

<script>
import { ethers } from 'ethers';
import { readableEther, getDisplayableBalanceList } from '../utils';
import TokenNameButton from './TokenNameButton.vue';
import CopyableAddress from './CopyableAddress.vue';
import CompleteOperationButton from './CompleteOperationButton.vue';

const components = {
    TokenNameButton,
    CopyableAddress,
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
        pendingOps: null,
    }),
    props: [
        // balances are like [{ tokenName: 'eth', amount: '120' }]
        'balances',
        'balanceListId'
    ],
    created() {
        this.updateBalanceList();
        this.updatePendingOps();
    },
    watch: {
        balances() {
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
        updateBalanceList() {
            if (this.balances != null) {
                this.displayableBalances = getDisplayableBalanceList(this.balances);
            }

            if (this.pendingOps != null) {
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
        updatePendingOps() {
            this.pendingOps = window.walletDecorator.pendingOperationsAsRenderableList();
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
