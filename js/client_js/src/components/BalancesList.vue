<template>
    <b-card title="Main chain">
        <b-col>
            <label for="ethereumAddressFormInput">Address</label> 
                (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+ethereumAddress"
                    target="blanc">block explorer</a>):
            <CopyableAddress id="ethereumAddressFormInput" :address="ethereumAddress"></CopyableAddress>
            <b-table borderless small responsive :fields="fields" :items="displayableBalances">
                <template v-slot:cell(tokenName)="data">
                    <TokenNameButton :data="data"></TokenNameButton>
                </template>
                <template v-slot:cell(amount)="data">
                    <span style="vertical-align: middle;"> {{ data.item.amount }} </span>
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

const components = {
    TokenNameButton,
    CopyableAddress,
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
        'balanceListId'
    ],
    watch: {
        balances() {
            this.displayableBalances = getDisplayableBalanceList(this.balances);
        },
    },
    methods: {
        clickedWhatever: function(evt) {
            let tgt = evt.target;
            tgt.setAttribute('data-original-title', 'copied');
            console.log(tgt);
        },
    },
    components,
}
</script>

<style scoped>
td:first-child {
    width: 2em;
}

.tokenNameButton {
    display: inline-block;
    height: 2;
}
</style>
