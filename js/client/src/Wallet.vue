<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-toggle target="nav_collapse"></b-navbar-toggle>
        <b-navbar-brand v-if="isDev" style="color: yellow;">{{apiServer}}</b-navbar-brand>
        <b-navbar-brand v-else>Matter Network Wallet <span style="font-size: 0.4em">ALPHA</span></b-navbar-brand>
        <b-collapse is-nav id="nav_collapse">
            <b-navbar-nav>
                <!-- <b-nav-item href="#" active>Account</b-nav-item> -->
                <!-- <b-nav-item href="#" disabled>Transactions</b-nav-item> -->
            </b-navbar-nav>
            <!-- Right aligned nav items -->
            <b-navbar-nav class="ml-auto">
                <span style="color: white">{{ store.account.franklinAddress }}</span>
            </b-navbar-nav>
        </b-collapse>
    </b-container>
    </b-navbar>
    <br>
    <b-container v-if="network && !correctNetwork">
        <h3 style="color: red">Please switch to <b>{{currentNetwork}}</b> network in Metamask to try this demo.</h3>
    </b-container>
    <b-container v-if="network && correctNetwork">
        <b-alert dismissible :variant="alertType" fade :show="countdown" @dismissed="countdown=0" class="mt-2">
            {{result}}
        </b-alert>

        <p>
            <b style="color: red">Warning</b>: this app is for demo only. Database and smart contracts will be reset from time to time, 
            with all coins lost!
        </p>
 
        <b-row>
            <b-col sm="6" order="2" class="col-xl-8 col-lg-7 col-md-6 col-sm-12">
                <b-card title="Transfer in Matter Network" class="mb-4 d-flex">


                    <label for="transferToInput">To (recepient ETH address): </label>
                    <b-form-input id="transferToInput" type="text" v-model="transferTo" placeholder="0x149e5ba19e2db1dbd58b54c088666c5a2f5b7fc4b8cf5c59614728" autocomplete="off"></b-form-input>
                    <p class="mt-2" style="color: grey">
                        For testing, try <a href="#" @click="transferTo=store.config.SENDER_ACCOUNT">0x{{store.config.SENDER_ACCOUNT}}</a>
                    </p>

                    <label for="transferAmountInput" class="mt-4">Amount</label>
<!--                            (max ETH <a href="#" @click="transferAmount=store.account.plasma.committed.balance">0</a>):-->
                    <b-form-input id="transferAmountInput" placeholder="10" type="number" v-model="transferAmount"></b-form-input>
                    <b-form-select v-model="selectedToken" :options="tokens"></b-form-select>

                    <div id="transferBtn" class="right">
<!--                        <img v-if="transferPending" style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">-->
                        <b-btn class="mt-4" variant="outline-primary" @click="transfer" >Submit transaction</b-btn>
                    </div>

<!--                    <p class="mt-2" style="color: grey">-->
<!--                        To commit a new block, either submit {{store.config.TX_BATCH_SIZE}} transactions, or wait 1 minute until timer triggers block generation.-->
<!--                    </p>-->
<!--                    <p class="mt-2" style="color: grey">-->
<!--                         Once a block is committed, it takes about 5 minutes to verify it.-->
<!--                    </p>-->
<!--                    <b-tooltip target="transferBtn" :disabled="transferPending" triggers="hover">-->
<!--                        Transfer not possible: Tx pending-->
<!--                    </b-tooltip>-->
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
                            <b-table class="mt-2" striped hover :items="store.account.ethBalances"></b-table>
                        </b-row>
                    </b-card>
                    <b-row class="mb-0 mt-0">
                        <b-col sm class="mb-2">
                            <div id="depositOnchainBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.depositOnchainModal >&#x21E9; Deposit</b-btn>
                            </div>
<!--                            <b-tooltip target="depositBtn" :disabled="!depositProblem" triggers="hover">-->
<!--                                Deposit not possible: {{ depositProblem }}-->
<!--                            </b-tooltip>-->
                        </b-col>
                        <b-col sm class="mb-2">
                            <div id="withdrawOnchainBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.withdrawOnchainModal >Withdraw &#x21E7;</b-btn>
                            </div>
<!--                            <b-tooltip target="withdrawBtn" :disabled="!withdrawProblem" triggers="hover">-->
<!--                                Withdrawal not possible: {{ withdrawProblem }}-->
<!--                            </b-tooltip>-->
                        </b-col>
                    </b-row>
                    <b-card class="mb-3">
                        <p class="mb-2"><strong>Gateway contract</strong></p>
                        <label for="addr">Address</label>
                        (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+store.account.address"
                            target="blanc">block explorer</a>):
                        <b-row class="mt-2">
                            <b-table class="mt-2" striped hover :items="store.account.contractBalances"></b-table>
                        </b-row>
<!--                        <b-row class="mt-2" style="color: grey" v-if="pendingWithdraw">-->
<!--                            <b-col cols="6">Pending:</b-col> <b-col>ETH {{store.account.onchain.balance}}</b-col>-->
<!--                        </b-row>-->
<!--                        <b-row class="mt-2 mx-auto" v-if="pendingWithdraw">-->
<!--                            <b-btn variant="primary" class="mt-2 mx-auto" @click="completeWithdraw">Complete withdrawal</b-btn>-->
<!--                        </b-row>-->
                    </b-card>
                    <b-row class="mb-0 mt-0">
                        <b-col sm class="mb-2">
                            <div id="depositOffchainBtn">
                                <b-btn variant="outline-primary" class="w-100"
                                       v-b-modal.depositOffchainModal >&#x21E9; Deposit</b-btn>
                            </div>
<!--                            <b-tooltip target="depositBtn" :disabled="!depositProblem" triggers="hover">-->
<!--                                Deposit not possible: {{ depositProblem }}-->
<!--                            </b-tooltip>-->
                        </b-col>
                        <b-col sm class="mb-2">
                            <div id="withdrawOffchainBtn">
                                <b-btn variant="outline-primary" class="w-100"
                                       v-b-modal.withdrawOffchainModal >Withdraw &#x21E7;</b-btn>
                            </div>
<!--                            <b-tooltip target="withdrawBtn" :disabled="!withdrawProblem" triggers="hover">-->
<!--                                Withdrawal not possible: {{ withdrawProblem }}-->
<!--                            </b-tooltip>-->
                        </b-col>
                    </b-row>
                    <b-card class="mt-2">
                        <p class="mb-2"><strong>Matter Network</strong>
                            (<a href="/explorer/" target="_blank">block explorer</a>)</p>

                        <label for="addr">Address</label>
                        <b-form-input id="addr" v-model="store.account.franklinAddress" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <!--                        <img src="./assets/loading.gif" width="100em" v-if="store.account.plasma.id === null">-->
<!--                        <div v-if="store.account.plasma.id === 0">-->
<!--                            <p>No account yet.</p>-->
<!--                        </div>-->
<!--                        <div v-if="store.account.plasma.id > 0 && store.account.plasma.closing">-->
<!--                            <p>Closing account #{{store.account.plasma.id}}: please complete pending withdrawal.</p>-->
<!--                        </div>-->
                        <div>
<!--                            <label for="acc_id">Account ID:</label>-->
<!--                            <b-form-input id="acc_id" v-model="store.account.plasma.id" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>-->
                            <b-row class="mt-2">
                                <b-col cols="8">Verified balance:</b-col> 
                                <b-table class="mt-2" striped hover :items="store.account.verifiedPlasmaBalances"></b-table>
                            </b-row>
                            <b-row class="mt-2" style="color: grey">
                                <b-col cols="8">Committed balance:</b-col> 
                                <b-table class="mt-2" striped hover :items="store.account.commitedPlasmaBalances"></b-table>
                            </b-row>
<!--                            <b-row class="mt-2">                    -->
<!--                                <b-col cols="8">Latest nonce:</b-col> -->
<!--                                <b-col>{{store.account.plasma.committed.nonce || 0}}</b-col>-->
<!--                            </b-row>-->
<!--                            <b-row class="mt-2" style="color: grey" v-if="store.account.plasma.pending.nonce !== store.account.plasma.committed.nonce">-->
<!--                                <b-col cols="8">Next nonce:</b-col> <b-col>{{store.account.plasma.pending.nonce || store.account.plasma.committed.nonce || 0}}</b-col>-->
<!--                            </b-row>-->
                        </div>
                    </b-card>
                </b-card>
            </b-col>
        </b-row>
    </b-container>

    <b-modal ref="depositOnchainModal" id="depositOnchainModal" title="Deposit onchain" hide-footer>
        <label for="depositAmountInput">Amount</label> 
        <b-form-input id="depositAmountInput" type="number" placeholder="10" v-model="depositAmount"></b-form-input>
        <b-form-select v-model="selectedToken" :options="tokens"></b-form-select>
        <div id="doDepositBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="depositOnchain" >Deposit</b-btn>
        </div>
    </b-modal>

    <b-modal ref="depositOffchainModal" id="depositOffchainModal" title="Deposit offchain" hide-footer>
        <label for="depositAmountInput">Amount</label>
        <b-form-input id="depositAmountInput" type="number" placeholder="10" v-model="depositAmount"></b-form-input>
        <b-form-select v-model="selectedToken" :options="tokens"></b-form-select>
        <div id="doDepositBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="depositOffchain" >Deposit</b-btn>
        </div>
    </b-modal>

    <b-modal ref="withdrawOnchainModal" id="withdrawOnchainModal" title="Withdraw onchain" hide-footer>
        <label for="withdrawAmountInput">Amount</label>
        <b-form-input id="withdrawAmountInput" type="number" placeholder="10" v-model="withdrawAmount"></b-form-input>
        <b-form-select v-model="selectedToken" :options="tokens"></b-form-select>
        <div id="doWithdrawOnchainBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="withdrawOnchain" >Withdraw</b-btn>
        </div>
    </b-modal>

    <b-modal ref="withdrawOffchainModal" id="withdrawOffchainModal" title="Withdraw offchain" hide-footer>
        <label for="withdrawAmountInput">Amount</label>
        <b-form-input id="withdrawAmountInput" type="number" placeholder="10" v-model="withdrawAmount"></b-form-input>
        <b-form-select v-model="selectedToken" :options="tokens"></b-form-select>
        <div id="doWithdrawOffchainBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="withdrawOffchain" >Withdraw</b-btn>
        </div>
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
import transactionLib from './transaction'

window.transactionLib = transactionLib

import ABI from './contract'

const maxExitEntries = 32;

export default {
    name: 'wallet',
    data: () => ({ 
        network:            null,

        nonce:              0,
        transferTo:         '',
        transferAmount:     '10',
        transferPending:    false,
        depositType: 'onchain',
        depositAmount:      '1',
        withdrawAmount:     null,

        selectedToken: {id: 0, address: '', symbol: 'ETH'},
        tokens: [{value: {id: 0, address: '', symbol: 'ETH'}, text: "ETH"}],

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
            url:    this.baseUrl + '/testnet_config',
        })
        if(!result.data) throw "Can not load contract address"
        store.contractAddress = result.data.address
        window.contractAddress = result.data.address

        console.log('contract: ', window.contractAddress)
        let contract = window.eth.contract(ABI).at(window.contractAddress)

        window.contract = contract

        window.ethersContract = new ethers.Contract(window.contractAddress, ABI, window.ethersProvider)
        //window.c = window.ethersContract

        this.updateAccountInfo()
        window.t = this
    },
    destroyed() {
    },
    computed: {
        isTestnet() {
            return this.network === '9'
        },
        currentNetwork() {
            return window.location.hostname.split('.')[0]
        },
        correctNetwork() {
            return this.isTestnet ||
                window.location.hostname.startsWith('localhost') ||
                (this.network === '1' && window.location.hostname.startsWith('mainnet')) ||
                (this.network === '4' && window.location.hostname.startsWith('rinkeby'))
        },
        baseUrl() {
            return this.apiServer + '/api/v0.1'
        },
        //baseUrl: () => 'https://api.plasma-winter.io',
        // baseUrl: () => 'https://api.Matter Network.thematter.io',
        store: () => store,
        contractAddress: () => window.contractAddress,
        pendingWithdraw: () => Number(store.account.onchain.balance) > 0,
    },
    methods: {
        async depositOnchain() {
            this.$refs.depositOnchainModal.hide()
            let amount;
            if (this.selectedToken.id == 0) {
              amount = ethers.utils.parseEther(this.depositAmount);
            } else {
                amount = ethers.utils.bigNumberify(this.depositAmount);
            }
            let tx_hash = await wallet.depositOnchain(this.selectedToken, amount);
            this.alert('Onchain deposit initiated, tx: ' + tx_hash, 'success')
        },
        async depositOffchain() {
            this.$refs.depositOffchainModal.hide()
            let amount;
            if (this.selectedToken.id == 0) {
                amount = ethers.utils.parseEther(this.depositAmount);
            } else {
                amount = ethers.utils.bigNumberify(this.depositAmount);
            }
            let tx_hash = await wallet.depositOffchain(this.selectedToken, amount, 0);
            this.alert('Offchain deposit initiated, tx: ' + tx_hash.hash, 'success')
        },
        async withdrawOnchain() {
            this.$refs.withdrawOnchainModal.hide()
            let amount;
            if (this.selectedToken.id == 0) {
                amount = ethers.utils.parseEther(this.withdrawAmount);
            } else {
                amount = ethers.utils.bigNumberify(this.withdrawAmount);
            }
            let tx_hash = await wallet.widthdrawOnchain(this.selectedToken, amount);
            this.alert('Onchain withdraw initiated, tx: ' + tx_hash, 'success')
        },
        async withdrawOffchain() {
            this.$refs.withdrawOffchainModal.hide()
            let amount;
            if (this.selectedToken.id == 0) {
                amount = ethers.utils.parseEther(this.withdrawAmount);
            } else {
                amount = ethers.utils.bigNumberify(this.withdrawAmount);
            }
            let tx= await wallet.widthdrawOffchain(this.selectedToken, amount, 0);
            this.alert('Offchain deposit initiated, tx: ' + tx.hash, 'success')
        },
        alert(msg, alertType) {
            this.result = msg
            this.countdown = 30
            this.alertType = alertType || 'danger'
        },
        async transfer() {
            let wallet = window.wallet;

            let amount;
            if (this.selectedToken.id == 0) {
                amount = ethers.utils.parseEther(this.transferAmount);
            } else {
                amount = ethers.utils.bigNumberify(this.transferAmount);
            }

            await wallet.transfer(this.transferTo, this.selectedToken, amount, 0);
        },
        async updateAccountInfo() {

            let newData = {}
            let timer = this.updateTimer
            let plasmaData = {}
            let onchain = {}
            let tokenSelectorData = []
            try {
                let wallet = window.wallet;

                await wallet.updateState();
                newData.address = wallet.ethAddress;
                newData.franklinAddress = wallet.address;
                newData.ethBalances = []
                newData.contractBalances = []
                newData.supportedTokens = []
                plasmaData.committedBalances = []
                plasmaData.verifiedBalances = []
                let commitedBalances = wallet.franklinState.commited.balances;
                let verifiedBalances = wallet.franklinState.verified.balances;

                for (let token of wallet.supportedTokens) {
                    let isEther = (token.id == 0);
                    function balanceToString(balance, isEther) {
                        if (isEther) {
                            return ethers.utils.formatEther(balance)
                        } else {
                            return balance.toString()
                        }
                    }
                    let tokenName = token.symbol || 'FNT';
                    newData.ethBalances.push({name: tokenName, balance: balanceToString(wallet.ethState.onchainBalances[token.id], isEther) })
                    newData.contractBalances.push({name: tokenName, balance: balanceToString(wallet.ethState.contractBalances[token.id], isEther) })
                    let commitedBalance = 0;
                    let verifidBalance = 0;
                    if (token.id in commitedBalances) {
                        commitedBalance = commitedBalances[token.id]
                    }
                    if (token.id in verifiedBalances) {
                        verifidBalance = verifiedBalances[token.id]
                    }
                    plasmaData.committedBalances.push({name: tokenName, balance: balanceToString(commitedBalance, isEther) });
                    plasmaData.verifiedBalances.push({name: tokenName, balance: balanceToString(verifidBalance, isEther) });
                    newData.supportedTokens.push({name: tokenName, token: token});
                    tokenSelectorData.push({value: token, text: tokenName});
                }
            } catch (err) {
                this.alert('Status update failed: ' + err)
                console.log(err)
            }
            if(timer === this.updateTimer) { // if this handler is still valid
                store.supportedTokens = newData.supportedTokens;
                store.account.address = newData.address
                store.account.franklinAddress = newData.franklinAddress
                store.account.ethBalances = newData.ethBalances;
                store.account.contractBalances = newData.contractBalances;

                store.account.commitedPlasmaBalances = plasmaData.committedBalances
                store.account.verifiedPlasmaBalances = plasmaData.verifiedBalances
                this.tokens = tokenSelectorData

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