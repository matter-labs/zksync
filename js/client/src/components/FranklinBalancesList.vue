<template>
    <b-card title="zkSync Devnet">
        <b-col>
            <img v-if="disabledReason == 'Not loaded'" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
            <p class="mt-3" v-else-if="disabledReason == 'No tokens'">
                <b>Your zkSync balance is empty.</b>
            </p>
            <b-table v-else class="b-table-balances-width-hack" borderless small responsive :fields="fields" :items="displayableBalances">
                <template v-slot:cell(tokenName)="data" style="width: 100px !important">
                    <TokenNameButton :data="data"></TokenNameButton>
                </template>
                <template v-slot:cell(amount)="data">
                    <span
                        style="vertical-align: middle;" 
                    > 
                        {{ data.item.committedAmount }} 
                        <span 
                            v-if="data.item.verified" 
                            style="color: #2a2;">
                            (Verified)
                        </span>
                        <span 
                            v-else 
                            style="color: #aaa"
                            v-b-tooltip.hover.left
                            :title="`last verified: ${data.item.verifiedAmount}`">
                            (verifying...)
                        </span>
                    </span>
                </template>
            </b-table>
        </b-col>
    </b-card>
</template>

<script>
import { readableEther, isReadablyPrintable, readablyPrintableTokens } from '../utils';

import TokenNameButton from './TokenNameButton.vue';

const components = {
    TokenNameButton,
};

export default {
    name: 'FranklinBalancesList',
    data: () => ({
        fields: [
            { key: 'tokenName', label: 'Token' }, 
            'amount',
        ],
        displayableBalances: [],
    }),
    props: [
        // balances are like [{ tokenName: 'eth', amount: '120' }]
        'balances',
        'balanceListId'
    ],
    created() {
        this.maybeUpdateInfo();
    },
    watch: {
        balances() {
            this.maybeUpdateInfo();
        },
    },
    computed: {
        disabledReason() {
            return this.balances == null      ? "Not loaded" 
                 : this.balances.length == 0  ? "No tokens" 
                 : null;
        },
    },
    methods: {
        maybeUpdateInfo() {
            if (this.balances == null) return;
            
            this.displayableBalances = this.balances
                .map(bal => {
                    if (isReadablyPrintable(bal.tokenName) == false) return bal;
                    let res = Object.assign({}, bal);
                    res.verifiedAmount = readableEther(res.verifiedAmount);
                    res.committedAmount = readableEther(res.committedAmount);
                    return res;
                })
                .filter(entry => Number(entry.committedAmount) || Number(entry.verifiedAmount));
        },
    },
    components,
}
</script>

<style>
.b-table-balances-width-hack td:first-child {
    width: 2em;
}
</style>
