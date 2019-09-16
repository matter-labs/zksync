<template>
<b-container>
    <b-row>
        <b-col xl="6">
            <BalancesList balanceListId="onchain" v-bind:balances="onchainBalances"></BalancesList>
            <DepositButtons 
                componentId="offchain"
                v-bind:topBalances="onchainBalances" 
                v-bind:bottomBalances="franklinBalances"
                v-bind:depositFeeNeeded="true"
                v-on:depositEvent="deposit"
                v-on:withdrawEvent="withdrawOffchain"
                ></DepositButtons>
            <FranklinBalancesList 
                balanceListId="franklin" 
                v-bind:balances="franklinBalancesWithInfo"
                ></FranklinBalancesList>
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
import FranklinBalancesList from './FranklinBalancesList.vue'
import DepositButtons from './DepositButtons.vue'
import Transfer from './Transfer.vue'

const components = {
    Alert,
    BalancesList,
    FranklinBalancesList,
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
        franklinBalancesWithInfo: [],
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
        async deposit(kwargs) {
            for await (const progress of window.walletDecorator.verboseDeposit(kwargs)) {
                console.log(progress);
                this.$emit('alert', {
                    message: progress.message,
                    variant: progress.error ? 'danger' : 'success',
                });
            }
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
