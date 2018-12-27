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
                        <b-col sm="2"><label for="transferToInput">To:</label></b-col>
                        <b-col sm="10"><b-form-input id="transferToInput" type="text" v-model="transferAmount" placeholder="0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0"></b-form-input></b-col>
                    </b-row>
                    <b-row class="mb-3">
                        <b-col sm="2"><label for="transferAmountInput">Amount:</label></b-col>
                        <b-col sm="4"><b-form-input id="transferAmountInput" type="number" placeholder="7.50"></b-form-input></b-col>
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
                        <b-col sm class="mb-2"><b-btn variant="outline-primary" class="w-100" v-b-modal.depositModal>&#x21E9; Deposit</b-btn></b-col>
                        <b-col sm class="mb-2"><b-btn variant="outline-primary" class="w-100" v-b-modal.withdrawModal>Withdraw &#x21E7;</b-btn></b-col>
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

    <b-modal ref="depositModal" id="depositModal" title="Deposit" hide-footer>
        <label for="depositAmountInput">Amount:</label>
        <b-form-input id="depositAmountInput" type="number" placeholder="7.50" v-model="depositAmount"></b-form-input>
        <b-btn variant="primary" @click="deposit" class="mt-4 float-right">Deposit</b-btn>
    </b-modal>

    <b-modal ref="withdrawModal" id="withdrawModal" title="Withdrawal" hide-footer>
        <b-form-checkbox plain v-model="withdrawAll" 
            aria-describedby="flavours"
            aria-controls="flavours">Withdraw all funds</b-form-checkbox>
        <div v-if="!withdrawAll">
            <label for="withdrawAmountInput">Amount:</label>
            <b-form-input id="withdrawAmountInput" type="number" placeholder="7.50" v-model="withdrawAmount"></b-form-input>
        </div>
        <b-btn variant="primary" @click="withdraw" class="mt-4 float-right">Withdraw</b-btn>
    </b-modal>
</div>
</template>

<script>

import store from './store'
import {BN} from 'bn.js'
import Eth from 'ethjs'
import axios from 'axios'

const baseUrl = 'http://188.166.33.159:8080'

export default {
    name: 'wallet',
    data: () => ({ 
        store,
        tx:             null,

        transferTo:     null,
        transferAmount: null,

        depositAmount:  null,

        withdrawAll:    true,
        withdrawAmount: null,

        updateInterval: null,
    }),
    created() {
        this.updateAccountInfo()
        this.updateInterval = setInterval(() => this.updateAccountInfo(), 1000)
    },
    destroyed() {
        clearInterval(this.updateInterval)
    },
    methods: {
        deposit() {
            this.$refs.depositModal.hide()
            let pub = store.account.plasma.key.publicKey
            let maxFee = new BN()

            console.log('deposit', this.depositAmount)
            let value = Eth.toWei(this.depositAmount, 'ether')
            let from = store.account.address
            console.log('deposit', value, from)

            contract.deposit([pub.x, pub.y], maxFee, { value, from })
        },
        withdraw() {
            this.$refs.withdrawModal.hide()
            console.log('withdraw', this.withdrawAll ? 'all' : this.withdrawAmount)
        },
        transfer() {
            console.log('transfer to', this.transferTo, this.transferAmount)
        },
        async updateAccountInfo() {
            try {
                let balance = (await eth.getBalance(store.account.address)).toString()
                store.account.balance = Eth.fromWei(balance, 'ether')

                let id = (await contract.ethereumAddressToAccountID(store.account.address))[0].toNumber()
                store.account.plasma.id = id

                if(id>0) {
                    const result = await axios({
                        method: 'get',
                        url: baseUrl + '/account/' + id,
                    });
                    let balance = new BN(result.data.balance).mul(new BN('1000000000000'))
                    store.account.plasma.balance = Eth.fromWei(balance, 'ether')
                    store.account.plasma.nonce = result.data.nonce
                }

            } catch (err) {
                console.log('status update failed: ', err)
            }
        },
    },
}
</script>
