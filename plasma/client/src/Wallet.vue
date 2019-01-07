<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-toggle target="nav_collapse"></b-navbar-toggle>
        <b-navbar-brand>Ignis Wallet <span style="font-size: 0.4em">ALPHA</span></b-navbar-brand>
        <b-collapse is-nav id="nav_collapse">
            <b-navbar-nav>
                <!-- <b-nav-item href="#" active>Account</b-nav-item> -->
                <!-- <b-nav-item href="#" disabled>Transactions</b-nav-item> -->
            </b-navbar-nav>
            <!-- Right aligned nav items -->
            <b-navbar-nav class="ml-auto">
                <span style="color: white">{{ store.account.address }}</span>
            </b-navbar-nav>
        </b-collapse>
    </b-container>
    </b-navbar>
    <br>
    <b-container v-if="network && !isTestnet">
        <h3 style="color: red">Please switch to Rinkeby network in Metamask to try this demo.</h3>
    </b-container>
    <b-container v-if="network && isTestnet">
        <b-alert show dismissible :variant="alertType" fade :show="countdown" @dismissed="countdown=0" class="mt-2">
            {{result}}
        </b-alert>
        <p>Warning: this software is for demo purposes only. Database and smart contracts might be reset from time to time, with all test coins lost!</p>
        <b-row>
            <b-col sm="6" order="2" class="col-xl-8 col-lg-7 col-md-6 col-sm-12">
                <b-card title="Transfer in Ignis" class="mb-4 d-flex">
                    <label for="transferToInput">To (recepient ETH address):</label>
                    <b-form-input id="transferToInput" type="text" v-model="transferTo" placeholder="0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0" autocomplete="off"></b-form-input>
                    <p class="mt-2" style="color: grey">Note: your recipient must register in Ignis first. For testing you can also use 0x6394b37cf80a7358b38068f0ca4760ad49983a1b, we're happy to accept your testcoins! :)</p>
                    <label for="transferAmountInput" class="mt-4">Amount</label>
                            (max ETH <a href="#" @click="transferAmount=store.account.plasma.committed.balance">{{store.account.plasma.committed.balance || 0}}</a>):
                    <b-form-input id="transferAmountInput" placeholder="7.50" type="number" v-model="transferAmount"></b-form-input>
                    <label for="transferNonceInput" class="mt-4">Nonce (autoincrementing):</label>
                    <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input>
                    <div id="transferBtn" class="right">
                        <img v-if="transferPending" style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">
                        <b-btn v-else class="mt-4" variant="outline-primary" @click="transfer" :disabled="!!transferProblem">Submit transaction</b-btn>
                    </div>
                    <p class="mt-2" style="color: grey">Balances will be updated once the block of 8 transactions is full and verified.</p>
                    <p class="mt-2" style="color: grey"> To force block generation, simply send at least 8 transactions in the correct nonce sequence.</p>
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
                            <b-col cols="6">Balance:</b-col> <b-col>ETH {{store.account.balance}}</b-col>
                        </b-row>
                        <b-row class="mt-2" style="color: grey" v-if="pendingWithdraw">
                           <b-col cols="6">Pending:</b-col> <b-col>ETH {{store.account.onchain.balance}}</b-col>
                        </b-row>
                        <b-row class="mt-2 mx-auto" v-if="pendingWithdraw">
                            <b-btn variant="primary" class="mt-2 mx-auto" @click="completeWithdraw">Complete withdrawal</b-btn>                            
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
                        <p class="mb-2"><strong>Ignis</strong>
                            (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+store.contractAddress"
                            target="blanc">contract</a>)</p>

                        <img src="./assets/loading.gif" width="100em" v-if="store.account.plasma.id === null">
                        <div v-if="store.account.plasma.id === 0">
                            <p>No account yet.</p>
                        </div>
                        <div v-if="store.account.plasma.id > 0 && store.account.plasma.closing">
                            <p>Closing account #{{store.account.plasma.id}}: please complete pending withdrawal.</p>
                        </div>
                        <div v-if="store.account.plasma.id > 0 && !store.account.plasma.closing">
                            <label for="acc_id">Account ID:</label>
                            <b-form-input id="acc_id" v-model="store.account.plasma.id" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                            <b-row class="mt-2">
                                <b-col cols="8">Verified balance:</b-col> 
                                <b-col>ETH {{store.account.plasma.verified.balance || 0}}</b-col>
                            </b-row>
                            <b-row class="mt-2" style="color: grey" v-if="store.account.plasma.verified.balance != store.account.plasma.committed.balance">
                                <b-col cols="8">Pending balance:</b-col> 
                                <b-col>ETH {{store.account.plasma.committed.balance || 0}}</b-col>
                            </b-row>
                            <b-row class="mt-2">                    
                                <b-col cols="8">Latest nonce:</b-col> 
                                <b-col>{{store.account.plasma.committed.nonce || 0}}</b-col>
                            </b-row>
                            <b-row class="mt-2" style="color: grey" v-if="store.account.plasma.pending.nonce !== store.account.plasma.committed.nonce">
                                <b-col cols="8">Next nonce:</b-col> <b-col>{{store.account.plasma.pending.nonce || store.account.plasma.committed.nonce || 0}}</b-col>
                            </b-row>
                        </div>
                    </b-card>
                </b-card>
            </b-col>
        </b-row>
    </b-container>

    <b-modal ref="depositModal" id="depositModal" title="Deposit" hide-footer>
        <label for="depositAmountInput">Amount</label> 
            (max ETH <a href="#" @click="depositAmount=store.account.balance">{{store.account.balance}}</a>):
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
                    (max ETH <a href="#" @click="withdrawAmount=store.account.plasma.verified.balance">{{store.account.plasma.verified.balance}}</a>):
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
                <p>This will close your account and withdraw all money from it.</p>
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
import {ethers} from 'ethers'
import axios from 'axios'
import ethUtil from 'ethjs-util'
import transactionLib from '../../contracts/lib/transaction.js'
import ABI from './contract'

const maxExitEntries = 32;

export default {
    name: 'wallet',
    data: () => ({ 
        network:            null,

        nonce:              0,
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
        this.network = web3.version.network

        console.log('start')
        let result = await axios({
            method: 'get',
            url:    this.baseUrl + '/details',
        })
        if(!result.data) throw "Can not load contract address"
        store.contractAddress = result.data.address
        window.contractAddress = result.data.address

        console.log('contract: ', window.contractAddress)
        let contract = window.eth.contract(ABI).at(window.contractAddress)

        window.contract = contract

        window.ethersContract = new ethers.Contract(window.contractAddress, ABI, window.ethersProvider)

        this.updateAccountInfo()
        window.t = this
    },
    destroyed() {
    },
    computed: {
        isTestnet() {
            return this.network === '4' || this.network === '1335'
        },
        //baseUrl: () => web3.version.network === '4' ? 'https://api.plasma-winter.io' : 'http://localhost:80',
        // baseUrl: () => 'https://api.plasma-winter.io',
        baseUrl: () => 'https://api.ignis.thematter.io',
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
            if(!(Number(store.account.plasma.committed.balance) > 0)) return "empty balance in the Ignis account"
            if(!(Number(store.account.plasma.verified.balance) > 0)) return "empty balance in the Ignis account"
        },
        doWithdrawProblem() {
            if(this.depositProblem) return this.depositProblem
            if(Number(this.withdrawAmount) > Number(store.account.plasma.committed.balance)) return "specified amount exceeds Ignis balance"
            if(Number(this.nonce) < Number(store.account.plasma.committed.nonce)) return "nonce must be greater then confirmed in Ignis: got " 
                + this.nonce + ", expected >= " + store.account.plasma.committed.nonce
        },
        transferProblem() {
            if(!store.account.plasma.id) return "no Ignis account exists yet"
            if(!(Number(store.account.plasma.committed.balance) > 0)) return "Ignis account has empty balance"
            if(!(Number(store.account.plasma.verified.balance) > 0)) return "empty balance in the Ignis account"
            if(!ethUtil.isHexString(this.transferTo)) return "`To` is not a valid ethereum address: " + this.transferTo
            if(!(this.transferAmount > 0)) return "positive amount required, e.g. 100.55"
            if(Number(this.transferAmount) > Number(store.account.plasma.committed.balance)) return "specified amount exceeds Ignis balance"
            if(Number(this.nonce) < Number(store.account.plasma.committed.nonce)) return "nonce must be greater then confirmed in Ignis: got " 
                + this.nonce + ", expected >= " + store.account.plasma.committed.nonce
        },
        pendingWithdraw: () => Number(store.account.onchain.balance) > 0,
    },
    methods: {
        async deposit() {
            this.$refs.depositModal.hide()
            let pub = store.account.plasma.key.publicKey
            let maxFee = new BN(0)
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
            try {
                // for now - one by one
                let from = store.account.address
                let maxIterations = store.account.onchain.completeWithdrawArgs;
                await contract.withdrawUserBalance(maxIterations, {from})
                this.updateAccountInfo();
            } catch(err) {
                this.alert('Exit request failed: ' + err)
            }
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
                url:        this.baseUrl + '/send',
                data:       apiForm
            });
            if(result.data.accepted) {
                this.alert(`Transaction with nonce #${this.nonce} accepted`, 'success')
                let new_nonce_result = await axios({
                        method: 'get',
                        url:    this.baseUrl + '/account/' + from,
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
            if (data.error !== undefined && data.error == "non-existent") {
                data.closing = true
            } else {
                data.closing = false
            }
            const multiplier = new BN('1000000000000')
            data.verified.balance = Eth.fromWei((new BN(data.verified.balance)).mul(multiplier), 'ether')
            data.committed.balance = Eth.fromWei((new BN(data.committed.balance)).mul(multiplier), 'ether')
            // TODO: remove when server updated
            if (Number(data.pending_nonce) > Number(data.pending.nonce)) {
                data.pending.nonce = data.pending_nonce
            }
            return data
        },
        async getPlasmaInfo(accountId) {
            //console.log(`getAccountInfo ${accountId}`)
            let result = (await axios({
                method: 'get',
                url:    this.baseUrl + '/account/' + accountId,
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
        async loadEvents(address, closing) {

            // let id = await ethersContract.ethereumAddressToAccountID(address);
            // if (id === 0) {
            //     return {blocks: [], pendingBalance: Eth.fromWei(new BN(0), 'ether')}
            // }
            // let accountInfo = await ethersContract.accounts(id);
   
            // if (accountInfo.exitListHead.toNumber() === 0) {
            //     return {blocks: [], pendingBalance: Eth.fromWei(new BN(0), 'ether')}
            // }

            // return {blocks: nonEmptyBlocks, pendingBalance}

            // let partialsFilter = contractForLogs.filters.LogExit(address, null)

            // let fullFilter = {
            //     fromBlock: 1,
            //     toBlock: 'latest',
            //     address: partialsFilter.address,
            //     topics: partialsFilter.topics
            // }

            // let events = await ethersProvider.getLogs(fullFilter)

            // if(closing) {
            //     let completeExitsFilter = contractForLogs.filters.LogCompleteExit(address, null)

            //     fullFilter = {
            //         fromBlock: 1,
            //         toBlock: 'latest',
            //         address: completeExitsFilter.address,
            //         topics: completeExitsFilter.topics
            //     }

            //     let completeExitEvents = await ethersProvider.getLogs(fullFilter)

            //     for (let i = 0; i < completeExitEvents; i++) {
            //         events.push(completeExitEvents[i]);
            //     }
            // }

            // const multiplier = new BN('1000000000000')
            // let finalBalance = new BN(0)
            // const nonEmptyBlocks = [];

            // for (let i = 0; i < events.length; i++) {
            //     let ev = events[i];
            //     let blockNumber = ethers.utils.bigNumberify(ev.topics[2]);
            //     let amount = (await contract.exitLeafs(address, blockNumber))[0];
            //     if (!amount.eq(new BN(0))) {
            //         let convertedBlockNumber = new BN(blockNumber.toString(10))
            //         nonEmptyBlocks.push(convertedBlockNumber);
            //         finalBalance = finalBalance.add(amount)
            //     }
            // }

            const multiplier = new BN('1000000000000')
            let finalBalance = new BN(0);

            let id = (await contract.ethereumAddressToAccountID(address))[0].toNumber();
            if (id === 0) {
                return {blocks: 0, pendingBalance: Eth.fromWei(new BN(0), 'ether')}
            }
            let accountInfo = await contract.accounts(id);
   
            if (accountInfo.exitListHead.toNumber() === 0) {
                // no entries
                return {blocks: 0, pendingBalance: Eth.fromWei(new BN(0), 'ether')}
            }

            let head = accountInfo.exitListHead;
            let entries = 0;
            for (let i = 0; i < maxExitEntries; i ++) {
                let entry = await contract.exitLeafs(address, head);
                finalBalance = finalBalance.add(entry.amount);
                entries = i + 1;
                if (entry.nextID.toNumber() === 0) {
                    break
                } else {
                    head = entry.nextID;
                }
            }

            let pendingBalance = Eth.fromWei(finalBalance.mul(multiplier), 'ether')
            return {blocks: entries, pendingBalance}
        },
        async updateAccountInfo() {

            //this.network = web3.version.network

            let newData = {}
            let timer = this.updateTimer
            let plasmaData = {}
            let onchain = {}
            try {
                newData.address = window.ethereum ? ethereum.selectedAddress : (await eth.accounts())[0]
                let balance = (await eth.getBalance(newData.address)).toString(10)
                newData.balance = Eth.fromWei(new BN(balance), 'ether')
                let id = (await contract.ethereumAddressToAccountID(newData.address))[0].toNumber();

                if( id !== store.account.plasma.id ) store.account.plasma.id = null // display loading.gif

                let accountState = await contract.accounts(id);

                // let accountState = await ethersContract.accounts(id);
                plasmaData.closing = accountState.state.toNumber() > 1;

                let {blocks, pendingBalance} = await this.loadEvents(newData.address, plasmaData.closing)
                onchain.completeWithdrawArgs = blocks
                onchain.balance = pendingBalance

                newData.plasmaId = id
                if(id > 0) {
                    plasmaData = await this.getPlasmaInfo(id)
                }
            } catch (err) {
                this.alert('Status update failed: ' + err)
                console.log(err)
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

                    if(store.account.plasma.pending.nonce !== null) {
                        
                        this.nonce = store.account.plasma.pending.nonce
                        
                        // if (store.account.plasma.pending.nonce > Number(this.nonce)) {
                        //     this.nonce = store.account.plasma.pending.nonce
                        // }
                        // if (store.account.plasma.pending_nonce > Number(this.nonce)) {
                        //     this.nonce = store.account.plasma.pending_nonce
                        // }
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