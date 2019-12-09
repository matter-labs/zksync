<template>
    <b-card title="Transfer in ZK Sync network" class="px-0">
        Address:
        <b-form-input autocomplete="off" type="text" v-model="address" class="mb-2"></b-form-input>
        <p>(for testing, use <code style="cursor: pointer" @click="address='0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e'">0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e</code>)</p>
        Choose token:
        <TokenSelector 
            class="mb-3"
            :tokens="tokensList"
            :selected.sync="token">
        </TokenSelector>
        Amount <span v-if="maxAmountVisible">(<span v-if="token == 'ETH'">in ETH tokens, </span>max {{ token }} {{ displayableBalancesDict[token] }})</span>:
        <b-form-input autocomplete="off" type="number" v-model="amountSelected" class="mb-3"></b-form-input>
        Choose fee:
        <FeeSelector 
            class="mb-3"
            :fees="fees"
            :selected.sync="feeButtonSelectedIndex">
        </FeeSelector>
        <img v-if="transferPending" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
        <b-button 
            v-else 
            :disabled="!!buttonDisabledReason"
            :title="buttonDisabledReason"
            class="mt-2 w-50" 
            variant="primary" 
            @click='buttonClicked'
        > Transfer </b-button>
    </b-card>
</template>

<script>
import { utils } from 'ethers'
import TokenSelector from './TokenSelector.vue'
import FeeSelector from './FeeSelector.vue'
import { getDisplayableBalanceDict, feesFromAmount, isReadablyPrintable } from '../utils';
import timeConstants from '../timeConstants'

const components = {
    TokenSelector,
    FeeSelector,
};

export default {
    name: 'Transfer',
    props: ['balances'],
    data: () => ({
        address: null,
        token: null,

        maxAmountVisible: false,
        balancesDict: {},
        tokensList: null,
        amountSelected: null,
        feeButtonSelectedIndex: null,
        fees: ['0%', '1%', '5%'],

        transferPending: false,
    }),
    created() {
        this.updateInfo();  
    },
    watch: {
        balances: function() {
            this.updateInfo();
        },
        token: function() {
            this.maxAmountVisible = true;
        },
    },
    computed: {
        buttonDisabledReason() {
            return this.balances == null      ? "Not loaded yet."
                :  this.balances.length == 0  ? "You have no tokens."
                :  null;
        },
    },
    methods: {
        updateInfo() {
            if (this.balances == null) return;
            
            this.balancesDict = this.balances
                .reduce((acc, bal) => {
                    acc[bal.tokenName] = bal.amount;
                    return acc;
                }, {});
            this.displayableBalancesDict = getDisplayableBalanceDict(this.balancesDict);
            this.tokensList = this.balances.map(bal => bal.tokenName);
        },
        localDisplayAlert(message) {
            this.$emit('alert', { message, variant: 'warning', countdown: 6 });
        },
        getAmount() {
            try {
                return isReadablyPrintable(this.token)
                    ? utils.parseEther(this.amountSelected)
                    : utils.bigNumberify(this.amountSelected);
            } catch (e) {
                console.log('amount compute error: ', e);
                return null;
            }
        },
        getFee() {
            try {
                let amount = this.getAmount();
                return feesFromAmount(amount)[this.feeButtonSelectedIndex];
            } catch (e) {
                return null;
            }
        },
        buttonClicked() {
            const addressLength = '0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e'.length;
            if (!this.address) {
                this.localDisplayAlert(`Please select address.`);
                return;
            }
            if (this.address.length != addressLength) {
                this.localDisplayAlert(`ZK Sync Devnet addresses are hex strings`
                    + `of length ${addressLength}. Are you sure this is a ZK Sync Devnet address?`);
                return;
            }
            if (this.address.startsWith('0x') === false) {
                this.localDisplayAlert(`ZK Sync Devnet addresses are hex strings starting with 0x`
                    + `Are you sure this is a ZK Sync Devnet address?`);
                return;
            }

            if (!this.token) {
                this.localDisplayAlert(`Please select token.`);
                return;
            }

            if (this.amountSelected == null) {
                this.localDisplayAlert(`Please select amount`);
                return;
            }

            let amount = this.getAmount();
            if (amount == null) {
                this.localDisplayAlert(`Please input valid amount value`);
                return;
            }

            if (this.feeButtonSelectedIndex == null) {
                this.localDisplayAlert(`Please select fee`);
                return;
            }

            let fee = this.getFee();
            if (fee == null) {
                this.localDisplayAlert(`Problem with fee.`);
                return;
            }

            if (amount.add(fee).gt(utils.bigNumberify(this.balancesDict[this.token]))) {
                this.localDisplayAlert(`It's too much, man!`);
                return;
            }

            this.transferPending = true;
            setTimeout(() => {
                this.transferPending = false;
            }, timeConstants.transferPending);

            this.$emit('buttonClicked', {
                address: this.address,
                token: this.token,
                amount: amount.toString(10),
                fee: fee.toString(10),
            });
        }
    },
    components,
}
</script>

<style scoped>
</style>
