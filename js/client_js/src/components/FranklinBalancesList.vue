<template>
    <b-card v-bind:title="balanceListId">
        <b-col>
            <b-row v-for="balance in balances" v-bind:key=" `${balanceListId}_${balance.tokenName}` ">
                <b-col class="col-sm-4"> {{ balance.tokenName }} </b-col>
                <b-col class="col-sm-4"> {{ balance.committedAmount }} </b-col>
                <b-col class="col-sm-4"> {{ balance.verifiedAmount }} </b-col>
            </b-row>
        </b-col>
    </b-card>
</template>

<script>
export default {
    name: 'BalancesList',
    props: [
        // balances are like [{ tokenName: 'eth', committedAmount: '130', verified, '140' }]
        'balances',
        'balanceListId'
    ],
}
</script>
