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
            <b-col sm="6" order="2" class="col-xl-8 col-lg-7 col-md-6 col-sm-12">
                <b-card title="Transfer in Plasma" class="mb-4 d-flex">
                    <b-alert show dismissible variant="success" fade :show="countdown" @dismissed="countdown=0" class="mt-2">
                        {{result}}
                    </b-alert>
                    <label for="transferToInput">To:</label>
                    <b-form-input id="transferToInput" type="text" v-model="transferTo" placeholder="0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0"></b-form-input>
                    <label for="transferAmountInput" class="mt-4">Amount</label>
                            (max <a href="#" @click="transferAmount=store.account.plasma.balance">{{store.account.plasma.balance || 0}}</a> ETH):
                    <b-form-input id="transferAmountInput" placeholder="7.50" type="number" v-model="transferAmount"></b-form-input>
                    <label for="transferNonceInput" class="mt-4">Nonce:</label>
                    <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input>
                    <div id="transferBtn" class="float-right mt-4">
                        <b-btn variant="outline-primary" @click="transfer" :disabled="!!transferProblem">Submit transaction</b-btn>
                    </div>
                    <b-tooltip target="transferBtn" :disabled="!transferProblem" triggers="hover">
                        Transfer not possible: {{ transferProblem }}
                    </b-tooltip>
                </b-card>

            </b-col>
            <b-col sm="6" class="col-xl-4 col-lg-5 col-md-6 col-sm-12 mb-5" order="1">
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
                        <p class="mb-2"><strong>Plasma</strong></p>
                        <label for="acc_id">Account ID:</label>
                        <b-form-input id="acc_id" v-model="store.account.plasma.id" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <b-row class="mt-2">
                            <b-col cols="4">Balance:</b-col> <b-col>{{store.account.plasma.balance || 0}} ETH</b-col>
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
        <label for="depositAmountInput">Amount</label> 
            (max <a href="#" @click="depositAmount=store.account.balance">{{store.account.balance}}</a> ETH):
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
                    (max <a href="#" @click="withdrawAmount=store.account.plasma.balance">{{store.account.plasma.balance}}</a> ETH):
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

window.eu = ethUtil

const baseUrl = 'http://188.166.33.159:8080'

export default {
    name: 'wallet',
    data: () => ({ 
        nonce:          0,
        transferTo:     '0x6394b37Cf80A7358b38068f0CA4760ad49983a1B',
        transferAmount: '0.001',
        depositAmount:  null,
        withdrawAmount: null,

        updateTimer:    0,
        countdown:      0,
        result:         null
    }),
    created() {
        console.log('start')
        this.updateAccountInfo()
        window.t = this
    },
    destroyed() {
    },
    computed: {
        store() { return store },
        depositProblem() {
            if(!(store.account.balance > 0)) return "empty balance in the mainchain account"
        },
        doDepositProblem() {
            if(this.depositProblem) return this.depositProblem
            if(!(this.depositAmount > 0)) return "invalid deposit amount: " + this.depositAmount
            if(Number(this.depositAmount) > Number(store.account.balance)) return "deposit amount exceeds mainchain account balance: " 
                + this.depositAmount + " > " + store.account.balance
        }, 
        withdrawProblem() {
            if(!(store.account.plasma.balance > 0)) return "empty balance in the Plasma account"
        },
        doWithdrawProblem() {
            if(this.depositProblem) return this.depositProblem
            if(Number(this.withdrawAmount) > Number(store.account.plasma.balance)) return "specified amount exceeds Plasma balance"
            if(Number(this.nonce) < Number(store.account.plasma.nonce)) return "nonce must be greater then confirmed in Plasma: got " 
                + this.nonce + ", expected >= " + store.account.plasma.nonce
        },
        transferProblem() {
            if(!store.account.plasma.id) return "no Plasma account exists yet"
            if(!(store.account.plasma.balance > 0)) return "Plasma account has empty balance"
            if(!ethUtil.isHexString(this.transferTo)) return "`To` is not a valid ethereum address: " + this.transferTo
            if(!(this.transferAmount > 0)) return "positive amount required, e.g. 100.55"
            if(Number(this.transferAmount) > Number(store.account.plasma.balance)) return "specified amount exceeds Plasma balance"
            if(Number(this.nonce) < Number(store.account.plasma.nonce)) return "nonce must be greater then confirmed in Plasma: got " 
                + this.nonce + ", expected >= " + store.account.plasma.nonce
        }
    },
    methods: {
        deposit() {
            this.$refs.depositModal.hide()
            let pub = store.account.plasma.key.publicKey
            let maxFee = new BN()
            let value = Eth.toWei(this.depositAmount, 'ether')
            let from = store.account.address
            contract.deposit([pub.x, pub.y], maxFee, { value, from })
        },
        async withdrawSome() {
            this.$refs.withdrawModal.hide()
            this.plasmaTransfer(0, this.withdrawAmount)
        },
        async withdrawAll() {
            this.$refs.withdrawModal.hide()
            console.log('full withdraw')
            await contract.exit()
            this.alert('full withdrawal initiated')
        },
        alert(msg) {
            this.result = msg
            this.countdown = 8
        },
        async transfer() {
            if(!ethUtil.isHexString(to)) {
                this.alert('to is not a hex string')
                return  
            }
            const to = (await contract.ethereumAddressToAccountID(this.transferTo))[0].toNumber()
            if(0 === to) {
                this.alert('recepient not found')
                return
            }
            this.plasmaTransfer(to, this.transferAmount)
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
            console.log(JSON.stringify(apiForm));
            const result = await axios({
                method:     'post',
                url:        baseUrl + '/send',
                data:       apiForm
            });
            console.log(JSON.stringify(result.data));
            this.alert(result.data)
        },
        async updateAccountInfo() {
            let newData = {}
            let timer = this.updateTimer
            try {
                newData.address = ethereum.selectedAddress

                let balance = (await eth.getBalance(newData.address)).toString()
                newData.balance = Eth.fromWei(balance, 'ether')

                let id = (await contract.ethereumAddressToAccountID(newData.address))[0].toNumber()
                newData.plasmaId = id

                if(id>0) {
                    const result = await axios({
                        method: 'get',
                        url:    baseUrl + '/account/' + id,
                    });
                    let balance = new BN(result.data.balance).mul(new BN('1000000000000'))
                    newData.plasmaBalance = Eth.fromWei(balance, 'ether')
                    newData.plasmaNonce = result.data.nonce
                }
            } catch (err) {
                //console.log('status update failed: ', err)
            }
            if(timer === this.updateTimer) { // if this handler is still valid
                store.account.address = newData.address
                store.account.balance = newData.balance
                store.account.plasma.id = newData.plasmaId
                store.account.plasma.balance = newData.plasmaBalance
                store.account.plasma.nonce = newData.plasmaNonce
                this.updateTimer = setTimeout(() => this.updateAccountInfo(), 1000)
            }
        },
    },
}
</script>
