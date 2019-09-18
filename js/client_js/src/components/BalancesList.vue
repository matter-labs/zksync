<template>
    <b-card :title="balanceListId">
        <b-col>
            <b-row v-for="balance in balances" :key=" `${balanceListId}_${balance.tokenName}` ">
                <b-col class="col-sm-3"> {{ balance.tokenName }} </b-col>
                <b-col class="col-sm-9"> {{ balance.amount }} </b-col>
            </b-row>
        </b-col>
    </b-card>
</template>

<script>
export default {
    name: 'BalancesList',
    props: [
        // balances are like [{ tokenName: 'eth', amount: '120' }]
        'balances',
        'balanceListId'
    ],
    computed: {
        balancesString: function() {
            return JSON.stringify(this.balances);
        }
    }
}
</script>
