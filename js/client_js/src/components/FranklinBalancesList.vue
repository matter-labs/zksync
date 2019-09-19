<template>
    <b-card title="Matter Testnet">
        <b-col>
            <label for="franklinAddressFormInput">Address</label> 
                (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+franklinAddress"
                    target="blanc">block explorer</a>):
            <CopyableAddress id="franklinAddressFormInput" :address="franklinAddress"></CopyableAddress>
            <b-table borderless small responsive :fields="fields" :items="balances">
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
        </b-col>
    </b-card>
</template>

<script>
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
    }),
    props: [
        // balances are like [{ tokenName: 'eth', amount: '120' }]
        'balances',
        'balanceListId'
    ],
    methods: {
        clickedWhatever: function(evt) {
            let tgt = evt.target;
            tgt.setAttribute('data-original-title', 'copied');
            console.log(tgt);
        }
    },
    components,
}
</script>

<style>
td:first-child {
    width: 2em;
}
</style>
