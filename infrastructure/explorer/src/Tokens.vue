<template>
    <div>
        <br />
        <b-container>
            <b-breadcrumb :items="breadcrumbs"></b-breadcrumb>
            <div v-if="loading">
                <img style="margin-right: 1.5em" src="./assets/loading.gif" width="100em" />
            </div>
            <div v-else>
                <h5>Supported Tokens</h5>
                <b-table responsive id="my-table" hover outlined :items="tokens" :fields="tokenFields" class="nowrap">
                    <template v-slot:cell(symbol)="data"><span v-html="data.item['symbol']" /></template>
                    <template v-slot:cell(address)="data">
                        <a
                            target="_blank"
                            rel="noopener noreferrer"
                            v-bind:href="`${urlForToken(data.item['address'])}`"
                        >
                            {{ data.item['address'] }}
                            <i class="fas fa-external-link-alt"></i>
                        </a>
                    </template>
                    <template v-slot:cell(decimals)="data"><span v-html="data.item['decimals']" /></template>
                    <template v-slot:cell(id)="data"><span v-html="data.item['id']" /></template>
                    <template v-slot:cell(acceptableForFees)="data">
                        <span v-if="data.item['acceptableForFees']">
                            <i class="far fa-credit-card fee-status green"></i>
                            Available
                        </span>
                        <span v-else>
                            <i class="fas fa-ban fee-status gray"></i>
                            <a class="unavailable-text">Unavailable</a>
                        </span>
                    </template>
                </b-table>
            </div>
        </b-container>
    </div>
</template>

<script>
import { clientPromise } from './Client';
import Navbar from './Navbar.vue';
import store from './store';
const components = {
    Navbar
};

export default {
    name: 'tokens',
    async created() {
        this.update();
    },
    activated() {
        this.update();
    },
    data() {
        return {
            tokens: [],
            loading: true,
            breadcrumbs: [
                {
                    text: 'Home',
                    to: '/'
                },
                {
                    text: 'Tokens',
                    active: true
                }
            ],
            tokenFields: [
                {
                    key: 'symbol'
                },
                {
                    key: 'address'
                },
                {
                    key: 'decimals'
                },
                {
                    key: 'id',
                    label: 'Internal Id'
                },
                {
                    key: 'acceptableForFees',
                    label: 'Can be used to pay fees'
                }
            ]
        };
    },
    methods: {
        async update() {
            const client = await clientPromise;
            this.tokens = await client.loadTokens();
            this.loading = false;
        },
        urlForToken(address) {
            const prefix = store.network === 'mainnet' ? '' : `${store.network}.`;
            return `https://${prefix}etherscan.io/token/${address}`;
        }
    },
    components
};
</script>

<style>
.fee-status {
    font-size: 1.25em;
    display: inline-block;
    margin-right: 5px;
}
.green {
    color: rgba(5, 122, 85, 1);
}
.gray {
    color: rgba(107, 114, 128, 1);
}
.unavailable-text {
    opacity: 0.75;
    color: #999 !important;
}
</style>
