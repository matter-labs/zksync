<template>
    <b-card title="Main chain">
        <b-col>
            <label for="ethereumAddressFormInput">Address</label> 
                (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+ethereumAddress"
                    target="blanc">block explorer</a>):
            <CopyableAddress id="ethereumAddressFormInput" :address="ethereumAddress"></CopyableAddress>
            <b-table borderless small responsive :fields="fields" :items="balances">
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

<style scoped>
td:first-child {
    width: 2em;
}

.tokenNameButton {
    display: inline-block;
    height: 2;
}
/* .copyable::before {
    display:block;
    transition: all 0.5s ease;
    position: absolute;
    transform: translate(-2em, -1em);
    content: "click to copy";
    opacity: 0;
}
.copyable:hover::before {
    background: yellow;
    opacity: 1;
} */
</style>
