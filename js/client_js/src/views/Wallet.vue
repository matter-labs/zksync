<template>
<b-row>
    <b-col sm="6" class="col-xl-4 col-lg-5 col-md-6 col-sm-12 mb-5">
        <Alert v-bind:message="message">asd</Alert>
        Onchain balances:
        <BalancesList balanceListId="onchain" v-bind:balances="onchainBalances"></BalancesList>
        Contract balances:
        <BalancesList balanceListId="contract" v-bind:balances="contractBalances"></BalancesList>
        <DepositButtons></DepositButtons>
        Franklin balances:
        <BalancesList balanceListId="franklin" v-bind:balances="franklinBalances"></BalancesList>
    </b-col>
    <b-col sm="6" class="col-xl-4 col-lg-5 col-md-6 col-sm-12 mb-5">
        else
    </b-col>
</b-row>
</template>

<script>
// TODO: remove this imports
import { ethers } from 'ethers'
import { Wallet, FranklinProvider } from 'franklin_lib'
import { WalletDecorator } from '../WalletDecorator'
// END-TODO

import Deposit from '../components/Deposit.vue'
import Alert from '../components/Alert.vue'
import BalancesList from '../components/BalancesList.vue'
import DepositButtons from '../components/DepositButtons.vue'

const components = {
    Deposit,
    Alert,
    BalancesList,
    DepositButtons
};

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export default {
    name: 'wallet',
    data: () => ({
        message: 'nn',
        onchainBalances: [],
        contractBalances: [],
        franklinBalances: [],
    }),
    async created() {
        // TODO: delete next block of code
        let franklinProvider = new FranklinProvider('http://localhost:3000', '0xc56E79CAA94C96DE01eF36560ac215cC7A4F0F47');
        // let signer = ethersProvider.getSigner();
        let provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
        window.signer = ethers.Wallet.fromMnemonic("fine music test violin matrix prize squirrel panther purchase material script deal").connect(provider);
        window.wallet = await Wallet.fromEthWallet(signer, franklinProvider);
        window.walletDecorator = new WalletDecorator(window.wallet);


        this.updateAccountInfo();
    },
    methods: {
        displayAlert(msg) {
            this.message = String(msg);
            alert(msg);
        },
        async updateAccountInfo() {
            await window.walletDecorator.updateState();
            this.onchainBalances = window.walletDecorator.onchainBalancesAsRenderableList();
            this.contractBalances = window.walletDecorator.contractBalancesAsRenderableList();
            this.franklinBalances = window.walletDecorator.franklinBalancesAsRenderableList();
            await sleep(2000);
            this.updateAccountInfo();
        }
    },
    components
}
</script>
