<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-toggle target="nav_collapse"></b-navbar-toggle>
        <b-navbar-brand>Plasma Wallet</b-navbar-brand>
        <b-collapse is-nav id="nav_collapse">
            <b-navbar-nav>
                <b-nav-item href="#" active>Account</b-nav-item>
                <b-nav-item href="#" disabled>Transactions</b-nav-item>
            </b-navbar-nav>
            <!-- Right aligned nav items -->
            <b-navbar-nav class="ml-auto">
                <b-nav-item right>{{ store.account.address }}</b-nav-item>
            </b-navbar-nav>
        </b-collapse>
    </b-container>
    </b-navbar>
    <br>
    <b-container class="bv-example-row">
        <b-row>
            <b-col sm="8" order="2">
                <b-card title="Transfer in Plasma" class="mb-4">
                    <b-row class="mb-3">
                        <b-col sm="2"><label for="to">To:</label></b-col>
                        <b-col sm="10"><b-form-input id="to" type="text"  placeholder="0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0"></b-form-input></b-col>
                    </b-row>
                    <b-row class="mb-3">
                        <b-col sm="2"><label for="amount">Amount:</label></b-col>
                        <b-col sm="4"><b-form-input id="amount" type="number" placeholder="7.50"></b-form-input></b-col>
                    </b-row>
                    <b-btn variant="outline-primary" @click="tx=2">Submit transaction</b-btn>
                    <b-alert show dismissible variant="success" fade :show="tx" @dismissed="tx=null" class="mt-2">
                        Submitted successfully
                    </b-alert>
                </b-card>
            </b-col>
            <b-col sm="4" class="mb-5" order="1">
                <b-card title="Account info">
                    <b-card class="mb-3">
                        <p class="mb-2"><strong>Mainchain</strong></p>

                        <label for="addr">Address:</label>
                        <b-form-input id="addr" v-model="store.account.address" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <b-row class="mt-2">
                            <b-col cols="4">Balance:</b-col> <b-col>{{store.account.balance}} ETH</b-col>
                        </b-row>
                    </b-card>
                    <b-row class="mb-0 mt-0">
                        <b-col sm class="mb-2"><b-btn variant="outline-primary" class="w-100">&#x21E9; Deposit</b-btn></b-col>
                        <b-col sm class="mb-2"><b-btn variant="outline-primary" class="w-100">Withdraw &#x21E7;</b-btn></b-col>
                    </b-row>
                    <b-card class="mt-2">
                        <p class="mb-2"><strong>Plasma</strong></p>
                        <label for="acc_id">Account ID:</label>
                        <b-form-input id="acc_id" v-model="store.account.plasma.id" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <b-row class="mt-2">
                            <b-col cols="4">Balance:</b-col> <b-col>{{store.account.plasma.balance}} ETH</b-col>
                        </b-row>
                        <b-row>                    
                            <b-col cols="4">Nonce:</b-col> <b-col>{{store.account.plasma.nonce}}</b-col>
                        </b-row>
                    </b-card>
                </b-card>
            </b-col>
        </b-row>
    </b-container>
</div>
</template>

<script>

import store from './store'

export default {
    name: 'wallet',
    data: () => ({ 
        store,
        tx: 1
    }),
    methods: {
    },
}
</script>
