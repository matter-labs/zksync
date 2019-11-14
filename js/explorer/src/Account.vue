<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
        <b-container>
            <b-navbar-brand href="/">Matter Network</b-navbar-brand>
        </b-container>
    </b-navbar>
    <b-container>
        <h5 class="mt-2">Account data</h5>
        <b-card no-body class="table-margin-hack">
            <b-table responsive outlined thead-class="hidden_header" class="my-0 py-0" :items="accountDataProps">
                <span slot="value" slot-scope="data" v-html="data.value"></span>
            </b-table>
        </b-card>
        <h5 class="mt-2">Account balances</h5>
        <b-card no-body class="table-margin-hack">
            <b-table responsive outlined thead-class="hidden_header" :fields="balancesFields" :items="balancesProps">
                <span slot="value" slot-scope="data" v-html="data.value"></span>
            </b-table>
        </b-card>
        <h5 class="mt-2">Account transactions</h5>
        <b-card no-body class="table-margin-hack">
            <b-table responsive outlined thead-class="hidden_header" :items="transactionProps">
                <span slot="value" slot-scope="data" v-html="data.value"></span>
            </b-table>
        </b-card>
    </b-container>
</div>
</template>

<style>
.hidden_header {
  display: none;
}
</style>

<script>

import store from './store'
import { FranklinProvider } from 'franklin_lib'
import config from './env-config'
import Axios from 'axios'
import { ethers } from 'ethers'

class walletDecorator {
    constructor(address) {
        this.address = address;
        this.fraProvider = new FranklinProvider(
            config.API_SERVER,
            config.CONTRACT_ADDR
        );
    }
    async getAccount() {
        return await Axios.get(`${config.API_SERVER}/api/v0.1/account/${this.address}`).then(r => r.data);
    }
    async getCommitedBalances() {
        let account = await this.getAccount();
        console.log(account);
        return Object.entries(account.commited.balances)
            .map(([tokenId, balance]) => {
                return { 
                    tokenId, balance,
                    balance: ethers.utils.formatEther(balance),
                    tokenName: ['ETH', 'DAI', 'FAU'][tokenId],
                };
            });
    }
    async getTransactions() {
        let [ offset, limit ] = [ 10, 10 ];
        return await this.fraProvider.getTransactionsHistory(this.address, offset, limit);   
    }
};

let client;

export default {
    name: 'Account',
    data: () => ({
        balances: [],
        transactions: [],
    }),
    async created() {
        client = new walletDecorator(this.address);

        this.update();
    },
    methods: {
        async update() {
            let balances = await client.getCommitedBalances();
            this.balances = balances
                .map(bal => Object.assign({ name: bal.tokenName, value: bal.balance }, bal));

            this.transactions = client.getTransactions();
        },
    },
    computed: {
        address() {
            return this.$route.params.address;
        },
        accountDataProps() {
            return [
                { name: 'Address',          value: `<code>${this.address}</code>`},
            ];
        },
        balancesFields() {
            return [
                'tokenName',
                'balance',
            ];
        },
        balancesProps() {
            return [
                ...this.balances,
            ];
        },
        transactionProps() {
            // optionally pass :fields="fields" to a btable
            return [
                ...this.transactions,
            ];
        },
    },
};
</script>

<style>
.round {
    border-radius: 3px;
    border: 1px solid #e5e5e5;
}
.table-margin-hack table {
    margin: 0 !important;
}
</style>
