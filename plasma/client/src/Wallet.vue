<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-toggle target="nav_collapse"></b-navbar-toggle>
        <b-navbar-brand>Plasma Wallet</b-navbar-brand>
        <b-collapse is-nav id="nav_collapse">
            <b-navbar-nav>
                <b-nav-item href="#">Send</b-nav-item>
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
            <b-col cols="8">
                <b-card header="Your account">
                    <b-form>
                        <b-form-input v-model="store.account" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                    </b-form>
                </b-card>
            </b-col>
            <b-col cols="4">
                <b-card header="Account info">
                    <b-row>
                        <b-col sm="5">Balance:</b-col> <b-col sm="6">0 ETH</b-col>
                    </b-row>
                    <b-row>                    
                        <b-col sm="5">Nonce:</b-col> <b-col sm="6">0</b-col>
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
