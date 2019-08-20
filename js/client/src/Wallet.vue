<template>
<div>
<!-- 
 ##     ## ########    ###    ########  ######## ########  
 ##     ## ##         ## ##   ##     ## ##       ##     ## 
 ##     ## ##        ##   ##  ##     ## ##       ##     ## 
 ######### ######   ##     ## ##     ## ######   ########  
 ##     ## ##       ######### ##     ## ##       ##   ##   
 ##     ## ##       ##     ## ##     ## ##       ##    ##  
 ##     ## ######## ##     ## ########  ######## ##     ## 
-->
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
                <span style="color: white">{{store.onchain.address}}</span>
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
<!-- 
   ###     ######   ######   #######  ##     ## ##    ## ########    #### ##    ## ########  #######  
  ## ##   ##    ## ##    ## ##     ## ##     ## ###   ##    ##        ##  ###   ## ##       ##     ## 
 ##   ##  ##       ##       ##     ## ##     ## ####  ##    ##        ##  ####  ## ##       ##     ## 
##     ## ##       ##       ##     ## ##     ## ## ## ##    ##        ##  ## ## ## ######   ##     ## 
######### ##       ##       ##     ## ##     ## ##  ####    ##        ##  ##  #### ##       ##     ## 
##     ## ##    ## ##    ## ##     ## ##     ## ##   ###    ##        ##  ##   ### ##       ##     ## 
##     ##  ######   ######   #######   #######  ##    ##    ##       #### ##    ## ##        #######  
 -->
            <b-col class="col-xl-6 mb-5" order="1">
                <b-card title="Account info">
                    <b-card class="mb-3">
                        <p class="mb-2"><strong>Mainchain</strong></p>
                        <label for="addr">Address</label> 
                            (<a v-bind:href="'https://rinkeby.etherscan.io/address/'+store.onchain.address"
                                target="blanc">block explorer</a>):
                        <b-form-input id="addr" v-model="ethereumAddress" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <b-row class="mt-2">
                            <b-col>Balances:
                                <b-row v-for="token in store.onchain.allTokensInfo" class="amount_show" v-bind:key="token.elemId" v-bind:id="token.elemId">
                                    <span v-bind:style="{color: token.color}">
                                        <span>{{token.shortBalanceString}}</span>
                                        <b-tooltip v-bind:target="token.elemId">{{token.onchainLongBalanceInfo}}</b-tooltip>
                                    </span>
                                </b-row>
                            </b-col>
                        </b-row>
                        <b-row class="mt-2 mx-auto" v-if="pendingWithdraw">
                            <b-btn variant="primary" class="mt-2 mx-auto" @click="completeWithdraw">Complete withdrawal</b-btn>                            
                        </b-row>
                    </b-card>
                    <b-row class="mb-0 mt-0">
                        <b-col sm class="mb-2">
                            <div id="onchainDepositBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.onchainDepositModal>&#x21E9; Deposit</b-btn>
                            </div>
                            <!-- <b-tooltip target="onchainDepositBtn" :disabled="!depositProblem" triggers="hover">
                                Deposit not possible: {{ depositProblem }}
                            </b-tooltip> -->
                        </b-col>
                        <b-col sm class="mb-2">
                            <div id="onchainWithdrawBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.onchainWithdrawModal>Withdraw &#x21E7;</b-btn>
                            </div>
                            <b-tooltip target="onchainWithdrawBtn" triggers="hover">
                                Withdrawal not possible: {{ withdrawProblem }}
                            </b-tooltip>
                        </b-col>
                    </b-row>
 <!-- 
 ######   #######  ##    ## ######## ########     ###     ######  ######## 
##    ## ##     ## ###   ##    ##    ##     ##   ## ##   ##    ##    ##    
##       ##     ## ####  ##    ##    ##     ##  ##   ##  ##          ##    
##       ##     ## ## ## ##    ##    ########  ##     ## ##          ##    
##       ##     ## ##  ####    ##    ##   ##   ######### ##          ##    
##    ## ##     ## ##   ###    ##    ##    ##  ##     ## ##    ##    ##    
 ######   #######  ##    ##    ##    ##     ## ##     ##  ######     ##
 -->    
                    <b-card class="mb-3">
                        <p class="mb-2"><strong>Contract</strong></p>
                        <b-row class="mt-2">
                            <b-col>Balances:
                                <b-row v-for="token in store.contract.allTokensInfo" class="amount_show" v-bind:key="token.elemId" v-bind:id="token.elemId">
                                    <span v-bind:style="{color: token.color}">
                                        <span>{{token.shortBalanceString}}</span>
                                        <b-tooltip v-bind:target="token.elemId">{{token.shortBalanceString}}</b-tooltip>
                                    </span>
                                </b-row>
                            </b-col>
                        </b-row>
                        <b-row class="mt-2 mx-auto" v-if="pendingWithdraw">
                            <b-btn variant="primary" class="mt-2 mx-auto" @click="completeWithdraw">Complete withdrawal</b-btn>                            
                        </b-row>
                    </b-card>
                    <b-row class="mb-0 mt-0">
                        <b-col sm class="mb-2">
                            <div id="depositBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.offchainDepositModal :disabled="!!depositProblem">&#x21E9; Deposit</b-btn>
                            </div>
                            <b-tooltip target="depositBtn" :disabled="!depositProblem" triggers="hover">
                                Deposit not possible: {{ depositProblem }}
                            </b-tooltip>
                        </b-col>
                        <b-col sm class="mb-2">
                            <div id="withdrawBtn">
                                <b-btn variant="outline-primary" class="w-100" 
                                    v-b-modal.offchainWithdrawModal :disabled="!!withdrawProblem">Withdraw &#x21E7;</b-btn>
                            </div>
                            <b-tooltip target="withdrawBtn" :disabled="!withdrawProblem" triggers="hover">
                                Withdrawal not possible: {{ withdrawProblem }}
                            </b-tooltip>
                        </b-col>
                    </b-row>
<!-- 
######## ########     ###    ##    ## ##    ## ##       #### ##    ## 
##       ##     ##   ## ##   ###   ## ##   ##  ##        ##  ###   ## 
##       ##     ##  ##   ##  ####  ## ##  ##   ##        ##  ####  ## 
######   ########  ##     ## ## ## ## #####    ##        ##  ## ## ## 
##       ##   ##   ######### ##  #### ##  ##   ##        ##  ##  #### 
##       ##    ##  ##     ## ##   ### ##   ##  ##        ##  ##   ### 
##       ##     ## ##     ## ##    ## ##    ## ######## #### ##    ## 
 -->
                    <b-card class="mt-2">
                        <p class="mb-2"><strong>Franklin</strong></p>
                        <label for="addr">Address</label> 
                            (<a v-bind:href="'/explorer/'+store.plasma.address" target="blanc">block explorer</a>):
                        <b-form-input id="franklin_addr" v-model="store.plasma.address" type="text" readonly bg-variant="light" class="mr-2"></b-form-input>
                        <b-row class="mt-2">
                            <b-col>Balances:
                                <b-row v-for="token in store.plasma.allTokensInfo" class="amount_show" v-bind:key="token.elemId" v-bind:id="token.elemId">
                                    <span v-bind:style="{color: token.color}">
                                        <span>{{token.shortBalanceString}}</span>
                                        <b-tooltip v-bind:target="token.elemId">{{token.shortBalanceString}}</b-tooltip>
                                    </span>
                                </b-row>
                            </b-col>
                        </b-row>
                        <b-row class="mt-2" style="color: grey" v-if="pendingWithdraw">
                           <b-col cols="6">Pending:</b-col> <b-col>ETH {{store.account.onchain.balance}}</b-col>
                        </b-row>
                        <b-row class="mt-2 mx-auto" v-if="pendingWithdraw">
                            <b-btn variant="primary" class="mt-2 mx-auto" @click="completeWithdraw">Complete withdrawal</b-btn>                            
                        </b-row>
                    </b-card>
                </b-card>
            </b-col>
<!-- 
######## ########     ###    ##    ##  ######  ######## ######## ########     ##     ##    ###    ######## ######## ######## ########  
   ##    ##     ##   ## ##   ###   ## ##    ## ##       ##       ##     ##    ###   ###   ## ##      ##       ##    ##       ##     ## 
   ##    ##     ##  ##   ##  ####  ## ##       ##       ##       ##     ##    #### ####  ##   ##     ##       ##    ##       ##     ## 
   ##    ########  ##     ## ## ## ##  ######  ######   ######   ########     ## ### ## ##     ##    ##       ##    ######   ########  
   ##    ##   ##   ######### ##  ####       ## ##       ##       ##   ##      ##     ## #########    ##       ##    ##       ##   ##   
   ##    ##    ##  ##     ## ##   ### ##    ## ##       ##       ##    ##     ##     ## ##     ##    ##       ##    ##       ##    ##  
   ##    ##     ## ##     ## ##    ##  ######  ##       ######## ##     ##    ##     ## ##     ##    ##       ##    ######## ##     ## 
 -->
            <b-col sm="6" order="2" class="col-xl-6">
                <b-card title="Transfer in Matter Network" class="mb-4 d-flex">
                    <label for="transferToInput">To (recepient Franklin address): </label>
                    <b-form-input id="transferToInput" type="text" v-model="transferTo" placeholder="0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0" autocomplete="off"></b-form-input>
                    <p class="mt-2" style="color: grey">
                    </p>
                    <label for="transferToken" class="mt-4">Token</label>
                    <b-form-select v-model="tokenToTransferFranklin" :option="store.plasma.allTokensList" class="mb-3">
                        <option v-for="token in store.plasma.allTokensInfo" v-bind:key="token.elemId" @click="tokenToTransferFranklin=token.tokenName">{{ token.tokenName   }}</option>
                    </b-form-select>
                    <label for="transferAmountInput" class="mt-4">Amount</label>
                            <!-- (max {{tokenToTransferFranklin}}&nbsp;<a href="#" @click="transferAmount=store.plasma.committed.balanceDict[tokenToTransferFranklin]">{{store.plasma.committed.balanceDict[tokenToTransferFranklin]}}</a>): -->
                    <b-form-input id="transferAmountInput" placeholder="7.50" type="number" v-model="transferAmount"></b-form-input>
                    <label for="transferNonceInput" class="mt-4">Fee:</label>
                    <b-form-input id="transferFeeInput" placeholder="0" type="number" v-model="transferFee"></b-form-input>

                    <!-- <label for="transferNonceInput" class="mt-4">Nonce (autoincrementing):</label>
                    <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input> -->

                    <div id="transferBtn" class="right">
                        <img v-if="transferPending" style="margin-right: 1.5em" src="./assets/loading.gif" width="100em">
                        <b-btn v-else class="mt-4" variant="outline-primary" @click="transfer" :disabled="!!transferProblem">Submit transaction</b-btn>
                    </div>

                    <p class="mt-2" style="color: grey">
                        To commit a new block, either submit {{store.config.TRANSFER_BATCH_SIZE}} transactions, or wait 1 minute until timer triggers block generation.
                    </p>
                    <p class="mt-2" style="color: grey">
                         Once a block is committed, it takes about 5 minutes to verify it.
                    </p>
                    <b-tooltip target="transferBtn" :disabled="transferPending || !transferProblem" triggers="hover">
                        Transfer not possible: {{ transferProblem }}
                    </b-tooltip>
                </b-card>

            </b-col>
        </b-row>
    </b-container>
<!-- 
########  ######## ########   #######   ######  #### ######## 
##     ## ##       ##     ## ##     ## ##    ##  ##     ##    
##     ## ##       ##     ## ##     ## ##        ##     ##    
##     ## ######   ########  ##     ##  ######   ##     ##    
##     ## ##       ##        ##     ##       ##  ##     ##    
##     ## ##       ##        ##     ## ##    ##  ##     ##    
########  ######## ##         #######   ######  ####    ##    
 -->
    <b-modal ref="onchainDepositModal" id="onchainDepositModal" title="Onchain deposit" hide-footer>
        <b-form-select v-model="tokenForDeposit.tokenName" class="mb-3">
            <option v-for="token in store.onchain.allTokensInfo" v-bind:key="token.elemId" @click="tokenForDeposit.amount=token.balance">{{ token.tokenName }}</option>
        </b-form-select>
        <label for="depositAmountInput">Amount</label> 
            (max <span>{{ tokenForDeposit.tokenName }}</span> <a href="#">{{ tokenForDeposit.amount }}</a>:
        <b-form-input id="depositAmountInput" type="number" placeholder="7.50" v-model="depositAmount"></b-form-input>
        <div id="doDepositBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="onchainDeposit" >Deposit</b-btn>
        </div>
        <b-tooltip target="doDepositBtn" :disabled="!doDepositProblem" triggers="hover">
            Deposit not possible: {{ doDepositProblem }}
        </b-tooltip>
    </b-modal>
    <b-modal ref="offchainDepositModal" id="offchainDepositModal" title="offchain deposit" hide-footer>
        <b-form-select v-model="tokenForDeposit.tokenName" class="mb-3">
            <option v-for="token in store.contract.allTokensInfo" v-bind:key="token.elemId" @click="tokenForDeposit.amount=token.balance">{{ token.tokenName }}</option>
        </b-form-select>
        <label for="depositAmountInput">Amount</label> 
            (max <span>{{ tokenForDeposit.tokenName }}</span> <a href="#">{{ tokenForDeposit.amount }}</a>:
        <b-form-input id="depositAmountInput" type="number" placeholder="7.50" v-model="depositAmount"></b-form-input>
        <label for="depositFeeInput">Fee</label> 
        <b-form-input id="depositFeeInput" type="number" placeholder="2" v-model="depositFee"></b-form-input>
        <div id="doDepositBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="offchainDeposit" >Deposit</b-btn>
        </div>
    </b-modal>
    <!-- <b-modal ref="depositModal" id="depositModal" title="Offchain deposit" hide-footer>
        <b-form-select v-model="tokenForDeposit.name" class="mb-3">
            <option v-for="token in store.contract.allTokensInfo" v-bind:key="token.elemId" @click="tokenForDeposit.amount=token.balance">{{ token.tokenName }}</option>
        </b-form-select>
        <label for="depositAmountInput">Amount</label> 
            (max <span>{{ tokenForDeposit.tokenName }}</span> <a href="#" @click="tokenForDeposit.amount=store.contract.committed.balanceDict[tokenForDeposit]">{{store.contract.committed.balanceDict[tokenForDeposit]}}</a>):
        <b-form-input id="depositAmountInput" type="number" placeholder="7.50" v-model="depositAmount"></b-form-input>
        <div id="doDepositBtn" class="mt-4 float-right">
            <b-btn variant="primary" @click="offchainDeposit" :disabled="!!doDepositProblem">Deposit</b-btn>
        </div>
        <b-tooltip target="doDepositBtn" :disabled="!doDepositProblem" triggers="hover">
            Deposit not possible: {{ doDepositProblem }}
        </b-tooltip>
    </b-modal> -->
<!-- 
##      ## #### ######## ##     ## ########  ########     ###    ##      ## 
##  ##  ##  ##     ##    ##     ## ##     ## ##     ##   ## ##   ##  ##  ## 
##  ##  ##  ##     ##    ##     ## ##     ## ##     ##  ##   ##  ##  ##  ## 
##  ##  ##  ##     ##    ######### ##     ## ########  ##     ## ##  ##  ## 
##  ##  ##  ##     ##    ##     ## ##     ## ##   ##   ######### ##  ##  ## 
##  ##  ##  ##     ##    ##     ## ##     ## ##    ##  ##     ## ##  ##  ## 
 ###  ###  ####    ##    ##     ## ########  ##     ## ##     ##  ###  ###  
 -->
    <b-modal ref="onchainWithdrawModal" id="onchainWithdrawModal" title="Onchains withdrawal" hide-footer>
        <b-tabs pills card>
            <b-form-select v-model="tokenForWithdrawal" :option="store.coins" class="mb-3">
                <option v-for="token in store.contract.allTokensInfo    " v-bind:key="token.elemId" @click="tokenForWithdrawal=token.tokenName">{{ token.tokenName }}</option>
            </b-form-select>

            <b-tab title="Partial withdrawal" active>
                <label for="withdrawAmountInput" class="mt-4">Amount</label>
                    <!-- (max <span>{{tokenForWithdrawal}}</span> <a href="#" @click="withdrawAmount=store.contract.committed.balanceDict[tokenForWithdrawal].toString(10)">{{store.contract.committed.balanceDict[tokenForWithdrawal].toString(10)}}</a>): -->
                <b-form-input id="withdrawAmountInput" type="number" placeholder="7.50" v-model="withdrawAmount"></b-form-input>
                <label for="transferNonceInput" class="mt-4">Nonce:</label>
                <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input>
                <div id="doWithdrawBtn" class="mt-4 float-right">
                    <b-btn variant="primary" @click="onchainWithdrawSome">Withdraw</b-btn>
                </div>
                <b-tooltip target="doWithdrawBtn" triggers="hover">
                    Withdraw not possible: {{ doWithdrawProblem }}
                </b-tooltip>
            </b-tab>
                <p>This will close your account and withdraw all money from it.</p>
                <div id="doExitBtn" class="mt-4 float-right">
                    <b-btn variant="danger" @click="withdrawAll">Close & withdraw</b-btn>
                </div>
                <b-tooltip target="doExitBtn" triggers="hover">
                    Withdraw not possible: {{ withdrawProblem }}
                </b-tooltip>
        </b-tabs>
    </b-modal>
    <b-modal ref="offchainWithdrawModal" id="offchainWithdrawModal" title="Offchains withdrawal" hide-footer>
        <b-tabs pills card>
            <b-form-select v-model="tokenForWithdrawal" :option="store.coins" class="mb-3">
                <option v-for="token in store.plasma.allTokensInfo    " v-bind:key="token.elemId" @click="tokenForWithdrawal=token.tokenName">{{ token.tokenName }}</option>
            </b-form-select>

            <b-tab title="Partial withdrawal" active>
                <label for="withdrawAmountInput" class="mt-4">Amount</label>
                    <!-- (max <span>{{tokenForWithdrawal}}</span> <a href="#" @click="withdrawAmount=store.plasma.committed.balanceDict[tokenForWithdrawal].toString(10)">{{store.plasma.committed.balanceDict[tokenForWithdrawal].toString(10)}}</a>): -->
                <b-form-input id="withdrawAmountInput" type="number" placeholder="7.50" v-model="withdrawAmount"></b-form-input>
                <label for="transferNonceInput" class="mt-4">Fee:</label>
                <b-form-input id="transferFeeInput" placeholder="0" type="number" v-model="transferFee"></b-form-input>
                <label for="transferNonceInput" class="mt-4">Nonce:</label>
                <b-form-input id="transferNonceInput" placeholder="0" type="number" v-model="nonce"></b-form-input>
                <div id="doWithdrawBtn" class="mt-4 float-right">
                    <b-btn variant="primary" @click="offchainWithdrawSome">Withdraw</b-btn>
                </div>
                <b-tooltip target="doWithdrawBtn" triggers="hover">
                    Withdraw not possible: {{ doWithdrawProblem }}
                </b-tooltip>
            </b-tab>
                <p>This will close your account and withdraw all money from it.</p>
                <div id="doExitBtn" class="mt-4 float-right">
                    <b-btn variant="danger" @click="withdrawAll">Close & withdraw</b-btn>
                </div>
                <b-tooltip target="doExitBtn" triggers="hover">
                    Withdraw not possible: {{ withdrawProblem }}
                </b-tooltip>
        </b-tabs>
    </b-modal>
</div>
</template>
<!--
 ######   ######  ########  #### ########  ######## 
##    ## ##    ## ##     ##  ##  ##     ##    ##    
##       ##       ##     ##  ##  ##     ##    ##    
 ######  ##       ########   ##  ########     ##    
      ## ##       ##   ##    ##  ##           ##    
##    ## ##    ## ##    ##   ##  ##           ##    
 ######   ######  ##     ## #### ##           ##    
-->

<script>

import store from './store'
import {BN} from 'bn.js'
import Eth from 'ethjs'
import {ethers} from 'ethers'
import axios from 'axios'
import ethUtil from 'ethjs-util'
import transactionLib from './transaction'
import * as Wallet from "../../franklin_lib/src/wallet"

window.transactionLib = transactionLib

import ABI from './contract'
import { setTimeout } from 'timers';
import { write } from 'fs';

const maxExitEntries = 32;

export default {
    name: 'wallet',
    data: () => ({ 
        network:            null,

        nonce:              0,
        tokenToTransferFranklin: 'ETH',
        tokenForDeposit:    { tokenName: '', balance: '', amount: '' },
        tokenForWithdrawal: {},
        transferFee:        0,
        depositFee:         0,
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
        let result;
        try {
            result = await axios({
                method: 'get',
                url:    this.baseUrl + '/testnet_config',
            });
        } catch (e) {
            // TODO: remove try/catch when server works again
            console.log("testnet_config still doesn't serve");
        }
        
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
/*
 *  ######   #######  ##     ## ########  ##     ## ######## ######## ########  
 * ##    ## ##     ## ###   ### ##     ## ##     ##    ##    ##       ##     ## 
 * ##       ##     ## #### #### ##     ## ##     ##    ##    ##       ##     ## 
 * ##       ##     ## ## ### ## ########  ##     ##    ##    ######   ##     ## 
 * ##       ##     ## ##     ## ##        ##     ##    ##    ##       ##     ## 
 * ##    ## ##     ## ##     ## ##        ##     ##    ##    ##       ##     ## 
 *  ######   #######  ##     ## ##         #######     ##    ######## ########  
 */

    computed: {
        franklinAccountActive() {
            return Math.random() < 0.3;
            // return store.account.plasma.id > 0 && !store.account.plasma.closing;
        },
        ethTokens() {
            return ['ETH', 'BTC', 'ZEC']
        },
        franklinCoins() {
            return ['ETH', 'BTC', 'ZEC'].reverse()
        },
        franklinAddress() {
            return wallet.address;
        },
        ethereumAddress() {
            return wallet.ethWallet.address;
        },
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
        depositProblem() {
            // if(store.account.plasma.closing) return "pending closing account, please complete exit first"
            // if ( ! store.onchain.committed.balances.some(b => b.balance > 0)) {
            //     return "empty balance in the mainchain account"
            // }
        },
        doDepositProblem() {
            // if(this.depositProblem) return this.depositProblem
            // if(!(this.depositAmount > 0)) return "invalid deposit amount: " + this.depositAmount
            // console.log('tokenForDEposit: ', this.tokenForDeposit);
            // let token = this.tokenForDeposit;
            // let onchainBalance = store.onchain.committed.balanceDict[token];
            // if(Number(this.depositAmount) > Number(onchainBalance)) return "deposit amount exceeds mainchain account balance: " 
            //     + this.depositAmount + " > " + onchainBalance;
        }, 
        withdrawProblem() {
            // console.log('store.plasma.committed', store.plasma.committed);
            // if(Object.keys(store.plasma.committed.balances).length === 0) return "empty balance in the Matter Network account"
            // // if(Object.keys(store.plasma.verified.balances).length === 0) return "empty balance in the Matter Network account"
        },
        doWithdrawProblem() {
            // if(this.depositProblem) return this.depositProblem
            // if(Number(this.withdrawAmount) > Number(store.account.plasma.committed.balance)) return "specified amount exceeds Matter Network balance"
            // if(Number(this.nonce) < Number(store.account.plasma.committed.nonce)) return "nonce must be greater then confirmed in Matter Network: got " 
            //     + this.nonce + ", expected >= " + store.account.plasma.committed.nonce
        },
        transferProblem() {
            // if ( false === store.plasma.committed.balances.some(b => b.balance > 0) ) {
            //     return "Matter Network account has empty balances"
            // }

            // // TODO: when prover works, uncomment
            // // console.log('store.plasma.verified', store.plasma.verified);
            // // if ( false === store.plasma.verified.balances.some(b => b.balance > 0) ) {
            // //     return "empty balance in the Matter Network account"
            // // }
            
            // // if(!ethUtil.isHexString(this.transferTo)) return "`To` is not a valid ethereum address: " + this.transferTo
            // if(!(this.transferAmount > 0)) return "positive amount required, e.g. 100.55"
            // // if(Number(this.transferAmount) > Number(store.account.plasma.committed.balance)) return "specified amount exceeds Matter Network balance"

            // let token = this.tokenToTransferFranklin;
            // let plasmaBalance = store.plasma.committed.balanceDict[token];
            // console.log('token trnasfer', token);
            // console.log('plasmaBalance for this token', plasmaBalance)

            // if ( this.transferAmount > plasmaBalance ) {
            //     return " insufficient funds for the operation ";
            // }
            
            // if(Number(this.nonce) < Number(store.plasma.committed.nonce)) return "nonce must be greater then confirmed in Matter Network: got " 
            //     + this.nonce + ", expected >= " + store.plasma.committed.nonce
        },
        pendingWithdraw: () => Number(store.account.onchain.balance) > 0,
    },
/*
 * ##     ## ######## ######## ##     ##  #######  ########   ######  
 * ###   ### ##          ##    ##     ## ##     ## ##     ## ##    ## 
 * #### #### ##          ##    ##     ## ##     ## ##     ## ##       
 * ## ### ## ######      ##    ######### ##     ## ##     ##  ######  
 * ##     ## ##          ##    ##     ## ##     ## ##     ##       ## 
 * ##     ## ##          ##    ##     ## ##     ## ##     ## ##    ## 
 * ##     ## ########    ##    ##     ##  #######  ########   ######  
 */
    methods: {
        nonverifiedFranklinTransaction(token) {
            return Math.random() < 0.2;
        },
        franklinCommittedBalanceForToken(token) {
            return Math.random() * 10000;
        },
        pendingFranklinTransaction(token) {
            return Math.random() < 0.2;
        },
        franklinPendingBalanceForToken(token) {
            return Math.random() * 10000;
        },
        pendingAmountForToken(token) {
            return (Math.random() * 1000).toPrecision(2);
        },
        pendingWithdrawForToken(token) {
            return Math.random() < 0.5;
        },
        onchainCommittedBalanceForToken(token) {
            // return res;
        },
        franklinBalanceForToken(token) {
            return 19937 + ({
                'ETH': 123,
                'BTC': 456,
                'ZEC': 789
            })[token];
        },
        async offchainDeposit() {
            this.$refs.offchainDepositModal.hide();
            try {
                this.alert('starting deposit offchain');
            
                let token = store.contract.allTokensDict[this.tokenForDeposit.tokenName].token;
                console.log('token for deposit: ', token);

                let amount = new BN(this.depositAmount);
                console.log('amount for deposit', amount);

                this.alert('starting deposit offchain');
                let fee = new BN(this.depositFee);
                console.log(amount)
                let res = await wallet.depositOffchain(token, amount, fee);

                this.alert("status of this transaction: " + JSON.stringify(res));

                let contract_balances = await wallet.getCommittedContractBalances();
            } catch (e) {
                this.alert('deposit failed: ' + e);
            }

            // let pub = store.account.plasma.key.publicKey
            // let maxFee = new BN(0)
            // let value = Eth.toWei(this.depositAmount, 'ether')
            // let from = store.account.address
            // let hash = await contract.deposit([pub.x, pub.y], maxFee, { value, from })
            // this.alert('Deposit initiated, tx: ' + hash, 'success')
        },
        async onchainDeposit() {
            this.$refs.onchainDepositModal.hide();

            this.alert('starting onchainDeposit');
            
            let token = store.onchain.allTokensDict[this.tokenForDeposit.tokenName].token;

            console.log('token for deposit: ', token);

            let amount = ethers.utils.bigNumberify(this.depositAmount);
            console.log('amount for deposit', amount);

            this.alert('starting deposit onchain');
            await wallet.depositOnchain(token, amount);

            this.alert('awaited deposit onchain');
        },
        async offchainWithdrawSome() {
            try {
                this.$refs.offchainWithdrawModal.hide();

                let token = (tokenName => {
                    for (let i = 0; i < wallet.supportedTokens.length; i++) {
                        let token = wallet.supportedTokens[i];
                        if (token.symbol == tokenName) {
                            return token;
                        }
                    }
                    throw new Error(`token not found, offchainWithdrawSome ${tokenName}`);
                })(this.tokenForWithdrawal);

                let fee = new BN(this.transferFee);

                let amount = new BN(this.withdrawAmount);

                this.alert(`offchainWithdraw ${token} ${amount}`);
                console.log('offchainWithdraw', token, amount)

                let res = await wallet.widthdrawOffchain(token, amount, fee);
                
                this.alert('offchainWithdraw res:', res);
                console.log('offchainWithdraw res:', res);
            } catch (e) {
                this.alert('offchainWithdraw error: ' + e);
            }
        },
        async onchainWithdrawSome() {
            try {
                this.$refs.onchainWithdrawModal.hide();

                let token = (tokenName => {
                    for (let i = 0; i < wallet.supportedTokens.length; i++) {
                        let token = wallet.supportedTokens[i];
                        if (token.symbol == tokenName) {
                            return token;
                        }
                    }
                    throw new Error(`token not found, onchainWithdrawSome ${tokenName}`);
                })(this.tokenForWithdrawal);

                let amount = ethers.utils.bigNumberify(this.withdrawAmount);

                this.alert(`onchainWithdraw ${token} ${amount}`);
                console.log('onchainWithdraw', token, amount)

                let res = await wallet.widthdrawOnchain(token, amount);
                
                this.alert('onchainWithdraw res:', res);
            } catch (e) {
                this.alert('onchainWithdraw error: ' + e);
            }
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
                // if(!ethUtil.isHexString(this.transferTo)) {
                //     this.alert('to is not a hex string')
                //     return 
                // }
                // const to = (await contract.ethereumAddressToAccountID(this.transferTo))[0].toNumber()
                // if(0 === to) {
                //     this.alert('recepient not found')
                //     return
                // }
                let to = this.transferTo;
                let token = this.tokenToTransferFranklin;
                console.log('token', token);
                console.log('store.plasma', store.plasma);
                token = store.plasma.allTokensDict[token].token;
                let amount = new BN(this.transferAmount);
                let fee = new BN(this.transferFee);
                
                await this.plasmaTransfer(token, to, amount, fee);
            } finally {
                this.transferPending = false
            }
        },
        async plasmaTransfer(token, to, amount, fee) {
            console.log('initiating transfer to', to, amount)

            let from = store.plasma.address;

            let res = await wallet.transfer(to, token, amount, fee);

            this.alert('transfer: ' + JSON.stringify(res));

            // const privateKey = store.account.plasma.key.privateKey
            // const nonce = this.nonce //store.account.plasma.nonce;
            // const good_until_block = 10000;
            // const fee = 0;

            // console.log(from, to, amount, fee, nonce, good_until_block, privateKey)

            // const apiForm = transactionLib.createTransaction(from, to, amount, fee, nonce, good_until_block, privateKey);
            // const result = await axios({
            //     method:     'post',
            //     url:        this.baseUrl + '/submit_tx',
            //     data:       apiForm
            // });
                // if(result.data.accepted) {
                //     this.alert(`Transaction with nonce #${this.nonce} accepted`, 'success')
                //     let new_nonce_result = await axios({
                //             method: 'get',
                //             url:    this.baseUrl + '/account/' + from,
                //         })
                //     if(!new_nonce_result.error) {
                //         let new_nonce = new_nonce_result.data.pending_nonce
                //         this.nonce = new_nonce
                //     } else {
                //         console.log('could not fetch data from server: ', new_nonce_result.error)
                //     }
                // } else  {
                //     this.alert(`Transaction rejected: ` + result.data.error)
                // }
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
            data.pending.balance = Eth.fromWei((new BN(data.pending.balance)).mul(multiplier), 'ether')
            // TODO: remove when server updated
            if (Number(data.pending_nonce) > Number(data.pending.nonce)) {
                data.pending.nonce = data.pending_nonce
            }
            return data
        },
        async getPlasmaInfo(accountId) {
            console.log('getplasmainfo called')
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
        async generateFranklinAddressss() {
            // let accounts = await eth.accounts()
            // console.log('Accounts: ', accounts);
            // let account = accounts[0]
            // this.acc = account
            // if (!account) {
            //     await ethereum.enable()
            //     account = ethereum.selectedAddress
            // }
            // console.log('Logging in with', account)
            // let sig = await eth.personal_sign(ethUtil.fromUtf8(new Buffer('Login Franklin v0.1')), account)
            // store.account.address = account

            // let hash = keccak256(sig)
            // console.log('sig', sig)
            // console.log('hash', hash)

            // store.account.plasma.key = newKey(sig)
            // console.log(store.account.plasma.key)

            // this.$parent.$router.push('/wallet')
        },
        async updateAccountInfo() {
            const zeroDecorator = store => k => store[k] !== undefined ? store[k] : 0;

            // *************** helper *****************
            const MaturityLevel = Object.freeze({
                low: 'pending',
                mid: 'committed',
                high: 'verified',

                toNumber: x => {
                    switch (x) {
                        case MaturityLevel.low: return 1;
                        case MaturityLevel.mid: return 2;
                        case MaturityLevel.high: return 3;
                        default: throw new Error('switch reached default');
                    }
                },
                cmp: (a, b) => {
                    MaturityLevel.toNumber(a) - MaturityLevel.toNumber(b)
                },
                lowestMaturityLevel: (pending, committed, verified) => {
                    if (pending !== committed) return MaturityLevel.low;
                    if (committed !== verified) return MaturityLevel.mid;
                    return MaturityLevel.high;
                },
                lowestFromList: xs => {
                    return xs.slice().sort(MaturityLevel.cmp)[0];
                }
            });
            
            let timer = this.updateTimer;
            let onchain = {};
            let contract = {};
            let plasma = {};
            try {
                // ******************* get the info from wallet *******************
                await wallet.fetchFranklinState();
                await wallet.fetchEthState();
                // console.log('wallet.franklinState', wallet.franklinState);
                // console.log('wallet.ethState', wallet.ethState);

                onchain.address = wallet.ethWallet.address;
                onchain.allTokensInfo = wallet.getCommittedOnchainState().onchainState;
                onchain.allTokensInfo = onchain.allTokensInfo.map(token => {
                    token.tokenName = token.token.symbol 
                        || 'erc20' + token.token.address;
                    token.elemId = `onchain_balance__${token.tokenName}`;
                    token.shortBalanceString = `${token.tokenName}: ${token.balance}`;
                    return token;
                });
                onchain.allTokensDict = {};
                onchain.allTokensInfo.map(info => onchain.allTokensDict[info.tokenName] = info)

                contract.allTokensInfo = wallet.getContractTokenInfo();
                contract.allTokensInfo = contract.allTokensInfo.map(token => {
                    token.tokenName = token.token.symbol 
                        || 'erc20' + token.token.address;
                    token.elemId = `contract_balance__${token.tokenName}`;
                    token.shortBalanceString = `${token.tokenName}: ${token.balance}, ${token.lockedBlocksLeft} left`;
                    return token;
                });
                contract.allTokensDict = {};
                contract.allTokensInfo.map(info => { 
                    contract.allTokensDict[info.tokenName] = info;
                });


                plasma.address = wallet.address;
                plasma.allTokensInfo = wallet.getFranklinTokensInfo(); 
                plasma.allTokensInfo = plasma.allTokensInfo.map(token => {
                    token.tokenName = token.token.symbol 
                        || 'erc20' + token.token.address;
                    token.elemId = `plasma_balance__${token.tokenName}`;
                    token.shortBalanceString = `${token.tokenName}: ${token.committedBalance}`;
                    return token;
                });
                plasma.allTokensDict = {};
                plasma.allTokensInfo.map(info => { 
                    plasma.allTokensDict[info.tokenName] = info;
                });
            } catch (err) {
                this.alert('Status update failed: ' + err)
                console.log(err)
            }
            if(timer === this.updateTimer) { // if this handler is still valid
                store.onchain = onchain;
                store.plasma = plasma;
                store.contract = contract;
                this.updateTimer = setTimeout(() => this.updateAccountInfo(), 1001)
            }
        },
    },
}

/*
 * ##     ## ######## ##       ########  ######## ########   ######  
 * ##     ## ##       ##       ##     ## ##       ##     ## ##    ## 
 * ##     ## ##       ##       ##     ## ##       ##     ## ##       
 * ######### ######   ##       ########  ######   ########   ######  
 * ##     ## ##       ##       ##        ##       ##   ##         ## 
 * ##     ## ##       ##       ##        ##       ##    ##  ##    ## 
 * ##     ## ######## ######## ##        ######## ##     ##  ######  
 */

function objectArrayToDict(arr, k, v) {
    let res = {};
    arr.forEach(b => {
        res[b[k]] = b[v]
    });
    return res;
}

function balanceArrayToDict(arr) {
    return objectArrayToDict(arr, "token", "balance");
}

function uniqueElements(arrays) {
    let res = {};
    arrays.forEach(arr => {
        arr.forEach(el => {
            res[el] = null;
        });
    });
    return Object.keys(res);
}
</script>

<style>
.pending {
    color: green!important;
}
.amount_show {
    overflow: auto;
    margin-left: 0;
    position: relative;
}
/* 
.amount_show::after {
    content: "";
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 3em;
    background: linear-gradient(to right, transparent, rgba(255, 255, 255, 0.7), white, white);
} */
</style>