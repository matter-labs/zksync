<template>
    <b-card title="Matter Testnet">
        <b-col>
            <label for="franklinAddressFormInput">Address</label> 
                <!-- (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+franklinAddress"
                    target="blanc">block explorer</a>): -->
            <CopyableAddress id="franklinAddressFormInput" :address="franklinAddress"></CopyableAddress>
            <b-table v-if="displayableBalances.length" class="b-table-balances-width-hack" borderless small responsive :fields="fields" :items="displayableBalances">
                <template v-slot:cell(tokenName)="data" style="width: 100px !important">
                    <TokenNameButton :data="data"></TokenNameButton>
                </template>
                <template v-slot:cell(amount)="data">
                    <span
                        style="vertical-align: middle;" 
                    > 
                        {{ data.item.committedAmount }} 
                        <span 
                            v-if="data.item.committedAmount == data.item.verifiedAmount" 
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
            <p class="mt-3" v-else>
                <b>Your Matter balance is empty.</b>
            </p>
        </b-col>
    </b-card>
</template>

<script>
import { formatUnits } from 'ethers/utils';
import { readableEther } from '../utils';

import TokenNameButton from './TokenNameButton.vue';
import CopyableAddress from './CopyableAddress.vue';

const components = {
    TokenNameButton,
    CopyableAddress,
};

export default {
    name: 'BalancesList',
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
        this.updateInfo();
    },
    watch: {
        balances() {
            this.updateInfo();
        },
    },
    methods: {
        updateInfo() {
            console.log("FranklinBalancesList updateInfo(), balances:", this.balances);
            this.displayableBalances = this.balances.map(bal => {
                if (bal.tokenName != 'ETH') return bal;
                let res = Object.assign({}, bal);
                res.verifiedAmount = readableEther(res.verifiedAmount);
                res.committedAmount = readableEther(res.committedAmount);
                return res;
            }).filter(entry => Number(entry.committedAmount) || Number(entry.verifiedAmount));
        },
        clickedWhatever: function(evt) {
            let tgt = evt.target;
            tgt.setAttribute('data-original-title', 'copied');
        }
    },
    components,
}
</script>

<style>
.b-table-balances-width-hack td:first-child {
    width: 2em;
}
</style>
