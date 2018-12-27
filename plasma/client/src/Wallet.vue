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
                <b-nav-item right>{{ store.account }}</b-nav-item>
            </b-navbar-nav>
        </b-collapse>
    </b-container>
    </b-navbar>
    <br>
    <b-container class="bv-example-row">
        <b-row>
            <b-col sm="8" class="mb-4">
                <b-card header="Your account">
                    <b-form>
                        <b-form-input v-model="store.account" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                    </b-form>
                </b-card>
            </b-col>
            <b-col sm="4" class="mb-4">
                <b-card header="Account info">
                    <b-row>
                        <b-col>Balance:</b-col> <b-col>0 ETH</b-col>
                    </b-row>
                    <b-row>                    
                        <b-col>Nonce:</b-col> <b-col>0</b-col>
                    </b-row>
                    <br>
                    <b-row>
                        <b-col sm class="mb-2"><b-btn variant="outline-secondary" class="w-100">Deposit</b-btn></b-col>
                        <b-col sm class="mb-2"><b-btn variant="outline-secondary" class="w-100">Withdraw</b-btn></b-col>
                    </b-row>
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
    data: () => ({ store }),
    methods: {
    },
}
</script>
