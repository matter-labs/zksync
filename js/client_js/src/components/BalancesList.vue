<template>
    <b-card title="Main chain">
        <b-col>
            <label for="ethereumAddressFormInput">Address</label> 
                (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+ethereumAddress"
                    target="blanc">block explorer</a>):
            <CopyableAddress id="ethereumAddressFormInput" :address="ethereumAddress"></CopyableAddress>
            <img v-if="loading" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
            <b-table v-else-if="displayableBalances.length" class="b-table-balances-width-hack" borderless small responsive :fields="fields" :items="displayableBalances">
                <template v-slot:cell(tokenName)="data">
                    <TokenNameButton :data="data"></TokenNameButton>
                </template>
                <template v-slot:cell(amount)="data">
                    <span style="vertical-align: middle;"> {{ data.item.amount }} </span>
                </template>
            </b-table>
            <p class="mt-3" v-else>
                <b>Your Main chain balance is empty.</b>
            </p>
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
        loading: true
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
            this.loading = false;
        },
    },
    methods: {
        updateInfo() {
            this.displayableBalances = getDisplayableBalanceList(this.balances);
        },
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
.tokenNameButton {
    display: inline-block;
    height: 2;
}
</style>
