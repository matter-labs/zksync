<template>
    <div>
        Token:
        <TokenSelector 
            class="mb-2"
            :tokens="tokensForTokenSelector"
            :selected.sync="token">
        </TokenSelector>
        <div v-if="depositFee == 'Deposit' && needAllowERC20">
            <p>For deposits to work, we need your approval for {{ token }}.</p>
            <img v-if="approveButtonStatus == 'loading' " style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
            <b-button v-else class="w-100 mt-3" variant="primary" @click='approveButtonClicked'> Approve </b-button>
        </div>
        <div v-else>
            Amount <span v-if="maxAmountVisible">(<span v-if="tokenReadablyPrintable">in {{ token }} coins, </span>max {{ displayableBalancesDict[token] }} {{ token }})</span>:
            <b-form-input autocomplete="off" v-model="amountSelected" class="mb-2"></b-form-input>
            <div v-if="feeNeeded">
                Fee:
                <FeeSelector 
                    class="mb-2"
                    :fees="fees"
                    :selected.sync="feeButtonSelectedIndex">
                </FeeSelector>
            </div>
            <div v-else>
                The fee is <b>ETH</b> {{ depositFee }}. The change will be put on your Matter account.
            </div>
            <p v-if="alertVisible"> {{ alertText }} </p>
            <b-button class="w-50 mt-3" variant="primary" @click='buttonClicked'> {{ buttonText }} </b-button>
        </div>
    </div>
</template>

<script>
import { bigNumberify, parseEther, formatUnits } from 'ethers/utils'
import { ethers } from 'ethers'
import { getDisplayableBalanceDict, feesFromAmount, isReadablyPrintable } from '../utils'

import TokenSelector from './TokenSelector.vue'
import FeeSelector from './FeeSelector.vue'

const NUMERIC_LIMITS_UINT_256 = '115792089237316195423570985008687907853269984665640564039457584007913129639935';

const components = {
    TokenSelector,
    FeeSelector,
};

export default {
    name: 'DepositWithdrawModal',
    props: [
        'buttonText',
        'balances',
        'feeNeeded',
    ],
    data: () => ({
        token: null,

        amountSelected: null,
        feeButtonSelectedIndex: null,
        fees: ['0%', '1%', '5%'],

        maxAmountVisible: false,
        balancesDict: {},
        displayableBalancesDict: {},
        alertVisible: false,
        alertText: '',
        depositFee: '',

        tokensForTokenSelector: null,

        allowances: null,
        allowancesDict: null,

        approveButtonStatusDict: {},
        approveButtonStatus: 'all done',

        needAllowERC20: false,
    }),
    async created() {
        this.depositFee = await window.walletDecorator.getDepositFee();
        this.createDisplayableBalancesDict();
        
        this.updateAllowances();
    },
    watch: {
        balances() {
            this.createDisplayableBalancesDict();
        },
        token() {
            this.maxAmountVisible = true;
            this.recomputeNeedAllowERC20();
        },
    },
    computed: {
        tokenReadablyPrintable() {
            return isReadablyPrintable(this.token);
        },
    },
    methods: {
        recomputeApproveButtonStatus() {
            this.approveButtonStatus = (
                this.approveButtonStatusDict[this.token] || 'all done'
            );
        },
        recomputeNeedAllowERC20() {
            this.needAllowERC20 = (
                  this.token == 'ETH'                                                    ? false
                : this.allowancesDict == null || this.allowancesDict[this.token] == null ? false
                : this.allowancesDict[this.token].toString().length != NUMERIC_LIMITS_UINT_256.length
            );
        },
        async updateAllowances() {
            this.allowances = await window.walletDecorator.allowancesForAllTokens();
            this.allowancesDict = this.allowances.reduce((acc, item) => {
                acc[item.token.id] = item.amount;
                acc[item.token.symbol || `erc20_${item.token.id}`] = item.amount;
                return acc;
            }, {});
            this.recomputeNeedAllowERC20();
        },
        async approveButtonClicked() {
            this.approveButtonStatusDict[this.token] = 'loading';
            this.recomputeApproveButtonStatus();

            try {
                let tx = await window.walletDecorator.wallet.approveERC20(
                    window.walletDecorator.tokenFromName(this.token), 
                    NUMERIC_LIMITS_UINT_256
                );

                this.allowancesDict[this.token] = NUMERIC_LIMITS_UINT_256;
                this.recomputeNeedAllowERC20();
            } catch (e) {
                console.log('error in approveButtonClicked', e);
            }

            this.approveButtonStatusDict[this.token] = 'all done';
            this.recomputeApproveButtonStatus();
        },
        localDisplayAlert(msg) {
            this.alertVisible = true;
            this.alertText = msg;
        },
        createDisplayableBalancesDict() {
            if (this.balances) {
                this.tokensForTokenSelector = this.balances.map(b => b.tokenName);

                this.balancesDict = this.balances
                    .reduce((acc, bal) => {
                        acc[bal.tokenName] = bal.amount;
                        return acc;
                    }, {});
                this.displayableBalancesDict = getDisplayableBalanceDict(this.balancesDict);
            }
        },
        getAmount() {
            try {
                return isReadablyPrintable(this.token)
                    ? parseEther(this.amountSelected)
                    : bigNumberify(this.amountSelected);
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
                console.log('getFee error:', e);
                return null;
            }
        },
        async buttonClicked() {
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

            if (this.feeNeeded) {
                if (this.feeButtonSelectedIndex == null) {
                    this.localDisplayAlert(`Please select fee`);
                    return;
                }
                
                var fee = this.getFee();
                if (fee == null) {
                    this.localDisplayAlert(`Problem with fee.`); // TODO:
                    return;
                }
    
                if (amount.add(fee).gt(bigNumberify(this.balancesDict[this.token]))) {
                    this.localDisplayAlert(`The amount is too large.`);
                    return;
                }
            } else {
                let fee = parseEther(this.depositFee);
                let tooMuch = (this.token == 'ETH' && amount.add(fee).gt(bigNumberify(this.balancesDict[this.token])))
                    || (amount.gt(bigNumberify(this.balancesDict[this.token])));

                if (tooMuch) {
                    this.localDisplayAlert(`The amount is too large.`);
                    return;
                }
            }

            this.$emit('buttonClicked', {
                token: this.token,
                amount: amount,
                fee: fee,
            });
        }
    },
    components,
}
</script>
