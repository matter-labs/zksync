<template>
<b-container>
    <b-row>
        <b-col xl="6">
            <b-container>
                <BalancesList balanceListId="onchain" v-bind:balances="onchainBalances"></BalancesList>
                <DepositButtons 
                    componentId="onchain"
                    v-bind:topBalances="onchainBalances" 
                    v-bind:bottomBalances="contractBalances"
                    v-on:depositEvent="depositOnchain"
                    v-on:withdrawEvent="withdrawOnchain"
                    ></DepositButtons>
                <BalancesList 
                    balanceListId="contract" 
                    v-bind:balances="contractBalances"
                    ></BalancesList>
                <DepositButtons 
                    componentId="offchain"
                    v-bind:topBalances="contractBalances" 
                    v-bind:bottomBalances="franklinBalances"
                    v-bind:depositFeeNeeded="true"
                    v-on:depositEvent="depositOffchain"
                    v-on:withdrawEvent="withdrawOffchain"
                    ></DepositButtons>
                <BalancesList 
                    balanceListId="franklin" 
                    v-bind:balances="franklinBalances"
                    ></BalancesList>
            </b-container>
        </b-col>
        <b-col xl="6">
            <Transfer
                v-bind:balances="franklinBalances"
                v-on:buttonClicked="transferFranklin"
                v-on:alert="displayAlert"
            ></Transfer>
        </b-col>
    </b-row>
</b-container>
</template>

<script>
import Alert from './Alert.vue'
import BalancesList from './BalancesList.vue'
import DepositButtons from './DepositButtons.vue'
import Transfer from './Transfer.vue'

const components = {
    Alert,
    BalancesList,
    DepositButtons,
    Transfer
};

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export default {
    name: 'wallet',
    props: ['info'],
    data: () => ({
        message: '',
        onchainBalances: [],
        contractBalances: [],
        franklinBalances: [],
    }),
    watch: {
        info: function() {
            for (let [key, val] of Object.entries(this.info)) {
                this[key] = val;
            }
        }
    },
    methods: {
        displayAlert(kwargs) {
            this.$emit('alert', kwargs);
        },
        async transferFranklin(kwargs) {
            console.log('transfer', kwargs);
            try {
                if ( ! window.walletDecorator) {
                    displayAlert({ message: `Wallet is ${window.walletDecorator}` });
                    return;
                }

                await window.walletDecorator.transfer(kwargs);
            } catch (e) {
                this.displayAlert({ message: `unknown error: ${e}` });
            }
        },
        async depositOnchain(kwargs) {
            console.log('depositOnchain', kwargs);
            try {
                if ( ! window.wallet) {
                    displayAlert({ message: `Wallet is ${window.walletDecorator}` });
                    return;
                }

                await window.walletDecorator.depositOnchain(kwargs);

                this.displayAlert({ message: `deposit succeeded or something` });
            } catch (e) {
                this.displayAlert({ message: `unknown error: ${e}` });
            }
        },
        async withdrawOnchain(kwargs) {
            console.log('withdrawOnchain', kwargs);
        },
        async depositOffchain(kwargs) {
            this.displayAlert({ message: `depositOffchain ${JSON.stringify(kwargs)}` });
            try {
                await window.walletDecorator.depositOffchain(kwargs);

                this.displayAlert({ message: `deposit succeeded or something`});
            } catch (e) {
                this.displayAlert(`unknown error: ${e}`);
            }
        },
        async withdrawOffchain(kwargs) {
            this.displayAlert({
                message: `withdrawOffchain ${JSON.stringify(kwargs)}`
            });
        },
    },
    components
}
</script>
