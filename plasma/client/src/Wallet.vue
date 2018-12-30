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
        <b-alert show dismissible :variant="alertType" fade :show="countdown" @dismissed="countdown=0" class="mt-2">
            {{result}}
        </b-alert>
        <b-row>
            <b-col sm="6" order="2" class="col-xl-8 col-lg-7 col-md-6 col-sm-12">
                <b-card title="Transfer in Plasma" class="mb-4 d-flex">
                    <label for="transferToInput">To:</label>
                    <b-form-input id="transferToInput" type="text" v-model="transferTo" placeholder="0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0"></b-form-input>
                    <label for="transferAmountInput" class="mt-4">Amount</label>
                            (max Ξ<a href="#" @click="transferAmount=store.account.plasma.committed.balance">{{store.account.plasma.committed.balance || 0}}</a>):
                    <b-form-input id="transferAmountInput" placeholder="7.50" type="number" v-model="transferAmount"></b-form-input>
                    <label for="transferNonceInput" class="mt-4">Nonce:</label>
                    <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input>
                    <div id="transferBtn" class="float-right">
                        <img v-if="transferPending" style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">
                        <b-btn v-else class="mt-4" variant="outline-primary" @click="transfer" :disabled="!!transferProblem">Submit transaction</b-btn>
                    </div>
                    <b-tooltip target="transferBtn" :disabled="transferPending || !transferProblem" triggers="hover">
                        Transfer not possible: {{ transferProblem }}
                    </b-tooltip>
                </b-card>

            </b-col>
            <b-col sm="6" class="col-xl-4 col-lg-5 col-md-6 col-sm-12 mb-5" order="1">
                <b-card title="Account info">
                    <b-card class="mb-3">
                        <p class="mb-2"><strong>Mainchain</strong></p>
                        <label for="addr">Address</label> 
                            (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+store.account.address"
                                target="blanc">block explorer</a>):
                        <b-form-input id="addr" v-model="store.account.address" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <b-row class="mt-2">
                            <b-col cols="6">Balance:</b-col> <b-col>Ξ{{store.account.balance}}</b-col>
                        </b-row>
                        <b-row class="mt-2" style="color: grey">
                           <b-col cols="6">Pending:</b-col> <b-col>Ξ{{store.account.onchain.balance}}</b-col>
                        </b-row>
                        <b-row class="mt-2 mx-auto">
                            <b-btn variant="primary" @click="completeWithdraw">Withdraw pending</b-btn>
                        </b-row>
                    </b-card>
                    <b-row class="mb-0 mt-0">
                        <b-col sm class="mb-2">
                            <div id="depositBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.depositModal :disabled="!!depositProblem">&#x21E9; Deposit</b-btn>
                            </div>
                            <b-tooltip target="depositBtn" :disabled="!depositProblem" triggers="hover">
                                Deposit not possible: {{ depositProblem }}
                            </b-tooltip>
                        </b-col>
                        <b-col sm class="mb-2">
                            <div id="withdrawBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.withdrawModal :disabled="!!withdrawProblem">Withdraw &#x21E7;</b-btn>
                            </div>
                            <b-tooltip target="withdrawBtn" :disabled="!withdrawProblem" triggers="hover">
                                Withdrawal not possible: {{ withdrawProblem }}
                            </b-tooltip>
                        </b-col>
                    </b-row>
                    <b-card class="mt-2">
                        <p class="mb-2"><strong>Plasma</strong>
                            (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+store.contractAddress"
                            target="blanc">contract</a>)</p>

                        <img src="./assets/loading.gif" width="100em" v-if="store.account.plasma.id === null">
                        <div v-if="store.account.plasma.id === 0">
                            <p>No account yet.</p>
                        </div>
                        <div v-if="store.account.plasma.closing">
                            <p>Closing account: please complete pending withdrawal.</p>
                        </div>
                        <div v-if="store.account.plasma.id > 0 && !store.account.plasma.closing">
                            <label for="acc_id">Account ID:</label>
                            <b-form-input id="acc_id" v-model="store.account.plasma.id" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                            <b-row class="mt-2">
                                <b-col cols="8">Verified balance:</b-col> 
                                <b-col>Ξ{{store.account.plasma.verified.balance || 0}}</b-col>
                            </b-row>
                            <b-row class="mt-2" style="color: grey" v-if="store.account.plasma.verified.balance != store.account.plasma.committed.balance">
                                <b-col cols="8">Committed balance:</b-col> 
                                <b-col>Ξ{{store.account.plasma.committed.balance || 0}}</b-col>
                            </b-row>
                            <b-row class="mt-2" v-if="store.account.plasma.pending.balance !== store.account.plasma.committed.balance" style="color: grey">
                            <!-- <b-row class="mt-2"> -->
                                <b-col cols="8">Next nonce:</b-col> 
                                <b-col>{{store.account.plasma.pending.nonce || 0}}</b-col>
                            </b-row>
                            <b-row class="mt-2">                    
                                <b-col cols="8">Mempool nonce:</b-col> <b-col>{{store.account.plasma.pending.nonce || 0}}</b-col>
                            </b-row>
                        </div>
                    </b-card>
                </b-card>
            </b-col>
        </b-row>
    </b-container>

    <b-modal ref="depositModal" id="depositModal" title="Deposit" hide-footer>
        <label for="depositAmountInput">Amount</label> 
            (max Ξ<a href="#" @click="depositAmount=store.account.balance">{{store.account.balance}}</a>):
        <b-form-input id="depositAmountInput" type="number" placeholder="7.50" v-model="depositAmount"></b-form-input>
        <div id="doDepositBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="deposit" :disabled="!!doDepositProblem">Deposit</b-btn>
        </div>
        <b-tooltip target="doDepositBtn" :disabled="!doDepositProblem" triggers="hover">
            Deposit not possible: {{ doDepositProblem }}
        </b-tooltip>
    </b-modal>

    <b-modal ref="withdrawModal" id="withdrawModal" title="Withdrawal" hide-footer>
        <b-tabs pills card>
            <b-tab title="Partial withdrawal" active>
                <label for="withdrawAmountInput" class="mt-4">Amount</label>
                    (max Ξ<a href="#" @click="withdrawAmount=store.account.plasma.verified.balance">{{store.account.plasma.verified.balance}}</a>):
                <b-form-input id="withdrawAmountInput" type="number" placeholder="7.50" v-model="withdrawAmount"></b-form-input>
                <label for="transferNonceInput" class="mt-4">Nonce:</label>
                <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input>
                <div id="doWithdrawBtn" class="mt-4 float-right">
                    <b-btn variant="primary"  :disabled="!!doWithdrawProblem" @click="withdrawSome">Withdraw</b-btn>
                </div>
                <b-tooltip target="doWithdrawBtn" :disabled="!doWithdrawProblem" triggers="hover">
                    Withdraw not possible: {{ doWithdrawProblem }}
                </b-tooltip>
            </b-tab>
            <b-tab title="Full exit" class="mb-4">
                <p>This will close your account and withdraw all money from it</p>
                <div id="doExitBtn" class="mt-4 float-right">
                    <b-btn variant="danger" :disabled="!!withdrawProblem" @click="withdrawAll">Close & withdraw</b-btn>
                </div>
                <b-tooltip target="doExitBtn" :disabled="!withdrawProblem" triggers="hover">
                    Withdraw not possible: {{ withdrawProblem }}
                </b-tooltip>
            </b-tab>
        </b-tabs>
    </b-modal>
</div>
</template>

<script>

import store from './store'
import {BN} from 'bn.js'
import Eth from 'ethjs'
import axios from 'axios'
import ethUtil from 'ethjs-util'
import transactionLib from '../../contracts/lib/transaction.js'
import ABI from './contract'

const baseUrl = 'https://api.plasma-winter.io'

export default {
    name: 'wallet',
    data: () => ({ 
        nonce:              null,
        transferTo:         '',
        transferAmount:     '0.001',
        transferPending:    false,
        depositAmount:      null,
        withdrawAmount:     null,

        updateTimer:        0,
        countdown:          0,
        alertType:          null,
        result:             null
    }),
    async created() {
        console.log('start')
        let result = await axios({
            method: 'get',
            url:    baseUrl + '/details',
        })
        if(!result.data) throw "Can not load contract address"
        store.contractAddress = result.data.address
        window.contractAddress = result.data.address
        window.contract = eth.contract(ABI).at(window.contractAddress)

        this.updateAccountInfo()
        window.t = this
    },
    destroyed() {
    },
    computed: {
        store: () => store,
        contractAddress: () => window.contractAddress,
        depositProblem() {
            if(store.account.plasma.closing) return "pending closing account, please complete exit first"
            if(!(Number(store.account.balance) > 0)) return "empty balance in the mainchain account"
        },
        doDepositProblem() {
            if(this.depositProblem) return this.depositProblem
            if(!(this.depositAmount > 0)) return "invalid deposit amount: " + this.depositAmount
            if(Number(this.depositAmount) > Number(store.account.balance)) return "deposit amount exceeds mainchain account balance: " 
                + this.depositAmount + " > " + store.account.balance
        }, 
        withdrawProblem() {
            if(!(Number(store.account.plasma.pending.balance) > 0)) return "empty balance in the Plasma account"
        },
        doWithdrawProblem() {
            if(this.depositProblem) return this.depositProblem
            if(Number(this.withdrawAmount) > Number(store.account.plasma.pending.balance)) return "specified amount exceeds Plasma balance"
            if(Number(this.nonce) < Number(store.account.plasma.pending.nonce)) return "nonce must be greater then confirmed in Plasma: got " 
                + this.nonce + ", expected >= " + store.account.plasma.pending.nonce
        },
        transferProblem() {
            if(!store.account.plasma.id) return "no Plasma account exists yet"
            if(!(store.account.plasma.pending.balance > 0)) return "Plasma account has empty balance"
            if(!ethUtil.isHexString(this.transferTo)) return "`To` is not a valid ethereum address: " + this.transferTo
            if(!(this.transferAmount > 0)) return "positive amount required, e.g. 100.55"
            if(Number(this.transferAmount) > Number(store.account.plasma.pending.balance)) return "specified amount exceeds Plasma balance"
            if(Number(this.nonce) < Number(store.account.plasma.pending.nonce)) return "nonce must be greater then confirmed in Plasma: got " 
                + this.nonce + ", expected >= " + store.account.plasma.pending.nonce
        }
    },
    methods: {
        async deposit() {
            this.$refs.depositModal.hide()
            let pub = store.account.plasma.key.publicKey
            let maxFee = new BN()
            let value = Eth.toWei(this.depositAmount, 'ether')
            let from = store.account.address
            let hash = await contract.deposit([pub.x, pub.y], maxFee, { value, from })
            this.alert('Deposit initiated, tx: ' + hash, 'success')
        },
        async withdrawSome() {
            try {
                this.transferPending = true
                this.$refs.withdrawModal.hide()
                await this.plasmaTransfer(0, this.withdrawAmount)
            } finally {
                this.transferPending = false
            }
        },
        async withdrawAll() {
            this.$refs.withdrawModal.hide()
            let from = store.account.address
            let hash = await contract.exit({ from })
            this.alert('Full exit initiated, tx: ' + hash, 'success')
        },
        async completeWithdraw() {
            // TODO AV: complete any type of withdrawal
            // await contract. ...(store.account.onchain.completeWithdrawArgs)
        },
        alert(msg, alertType) {
            this.result = msg
            this.countdown = 30
            this.alertType = alertType || 'danger'
        },
        async transfer() {
            try {
            this.transferPending = true
                if(!ethUtil.isHexString(this.transferTo)) {
                    this.alert('to is not a hex string')
                    return  
                }
                const to = (await contract.ethereumAddressToAccountID(this.transferTo))[0].toNumber()
                if(0 === to) {
                    this.alert('recepient not found')
                    return
                }
                await this.plasmaTransfer(to, this.transferAmount)
            } finally {
                this.transferPending = false
            }
        },
        async plasmaTransfer(to, amount) {
            console.log('initiating transfer to', to, amount)

            const from = store.account.plasma.id

            amount = Eth.toWei(amount, 'ether').div(new BN('1000000000000')).toNumber();

            const privateKey = store.account.plasma.key.privateKey
            const nonce = this.nonce //store.account.plasma.nonce;
            const good_until_block = 100;
            const fee = 0;

            console.log(from, to, amount, fee, nonce, good_until_block, privateKey)

            const apiForm = transactionLib.createTransaction(from, to, amount, fee, nonce, good_until_block, privateKey);
            const result = await axios({
                method:     'post',
                url:        baseUrl + '/send',
                data:       apiForm
            });
            if(result.data.accepted) {
                this.alert(`Transaction with nonce #${this.nonce} accepted`, 'success')
                let new_nonce_result = await axios({
                        method: 'get',
                        url:    baseUrl + '/account/' + from,
                    })
                if(!new_nonce_result.error) {
                    let new_nonce = new_nonce_result.data.pending_nonce
                    this.nonce = new_nonce
                } else {
                    console.log('could not fetch data from server: ', new_nonce_result.error)
                }
            } else  {
                this.alert(`Transaction rejected!`)
            }
        },
        parseStateResult(data) {
            data.verified.balance = Eth.fromWei(new BN(data.verified.balance).mul(new BN('1000000000000')), 'ether')
            data.committed.balance = Eth.fromWei(new BN(data.committed.balance).mul(new BN('1000000000000')), 'ether')
            data.pending.balance = Eth.fromWei(new BN(data.pending.balance).mul(new BN('1000000000000')), 'ether')
            if (Number(data.pending_nonce) > Number(data.pending.nonce)) {
                // TODO: remove when server updated
                data.pending.nonce = data.pending_nonce
            }
            return data
        },
        async getPlasmaInfo(accountId) {
            //console.log(`getAccountInfo ${accountId}`)
            let result = (await axios({
                method: 'get',
                url:    baseUrl + '/account/' + accountId,
            }))
            if(result.status !== 200) {
                throw `Could not load data for account ${accountId}: ${result.error}`
            }
            if(result.data.error === 'non-existing account') {
                return { closing: true }
            }
            if(result.data.error) {
                throw `Getting data for account ${accountId} failed: ${result.data.error}`
            }
            return this.parseStateResult(result.data)
        },
        async updateAccountInfo() {
            let newData = {}
            let timer = this.updateTimer
            let plasmaData = {}
            let onchain = {}
            try {
                newData.address = ethereum.selectedAddress
                let balance = (await eth.getBalance(newData.address)).toString()
                newData.balance = Eth.fromWei(balance, 'ether')
                let id = (await contract.ethereumAddressToAccountID(newData.address))[0].toNumber()

                if( id !== store.account.plasma.id ) store.account.plasma.id = null // display loading.gif

                onchain.account = await contract.accounts(id)

                // TODO AV: read events and fill values below:
                onchain.balance = 5 // available to complete withdraw
                onchain.completeWithdrawArgs = {} // arguments to use in this.completeWithdraw()

                newData.plasmaId = id
                if(id > 0) {
                    plasmaData = await this.getPlasmaInfo(id)
                }
            } catch (err) {
                this.alert('Status update failed: ' + err)
            }
            if(timer === this.updateTimer) { // if this handler is still valid
                store.account.address = newData.address
                store.account.balance = newData.balance

                store.account.onchain = onchain

                store.account.plasma.id = newData.plasmaId
                store.account.plasma.closing = plasmaData.closing

                if(store.account.plasma.id) {

                    //console.log('plasmaData', plasmaData)
                    store.account.plasma.verified = plasmaData.verified || {}
                    store.account.plasma.committed = plasmaData.committed || {}
                    store.account.plasma.pending = plasmaData.pending || {}

                    if(store.account.plasma.pending.nonce) {
                        if (store.account.plasma.pending.nonce > Number(this.nonce)) {
                            this.nonce = store.account.plasma.pending.nonce
                        }
                    }
                }
                this.updateTimer = setTimeout(() => this.updateAccountInfo(), 1000)
            }
        },
    },
}
</script>

<style>
.pending {
    color: green!important;
}
</style>