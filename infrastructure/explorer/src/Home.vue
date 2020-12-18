<template>
    <div>
        <br />
        <b-container>
            <b-alert v-if="updateError" variant="danger" show> {{ updateError }}. Try again later. </b-alert>
            <b-card bg-variant="light">
                <h4>zkSync {{ store.capitalizedNetwork }} Block Explorer</h4>
                <SearchField :searchFieldInMenu="false" />
            </b-card>
            <br />
            <b-card>
                <div class="row" style="color: grey">
                    <div class="col-sm text-center">
                        <i class="far fa-square"></i> <b>Blocks committed</b><br /><span class="num">{{
                            lastCommitted
                        }}</span>
                    </div>
                    <div class="col-sm text-center">
                        <i class="far fa-check-square"></i> <b>Blocks verified</b>
                        <br />
                        <span class="num">{{ lastVerified }}</span>
                    </div>
                    <div class="col-sm text-center">
                        <i class="fas fa-list"></i> <b>Total transactions</b><br /><span class="num">{{
                            totalTransactions
                        }}</span>
                    </div>
                </div>
            </b-card>
            <br />

            <b-pagination
                v-if="ready && totalTransactions > perPage"
                v-model="currentPage"
                :per-page="perPage"
                :total-rows="rows"
                @change="onPageChanged"
            ></b-pagination>
            <b-table responsive class="nowrap" hover outlined :items="items" :busy="loading">
                <template v-slot:cell(block_number)="data">
                    <Entry :value="data.item.block_number.value" class="block_number_link" />
                </template>
                <template v-slot:cell(status)="data">
                    <Entry :value="data.item.status.value" />
                </template>
                <template v-slot:cell(new_state_root)="data">
                    <Entry :value="data.item.new_state_root.value" />
                </template>
                <template v-slot:cell(accepted_at)="data">
                    <Entry :value="data.item.accepted_at.value" />
                </template>
                <template v-slot:cell(verified_at)="data">
                    <Entry :value="data.item.verified_at.value" />
                </template>
            </b-table>
            <b-pagination
                v-if="ready && totalTransactions > perPage"
                v-model="currentPage"
                :per-page="perPage"
                :total-rows="rows"
                @change="onPageChanged"
            ></b-pagination>
        </b-container>
    </div>
</template>

<script>
import * as constants from './constants';
import { clientPromise } from './Client';
import SearchField from './SearchField.vue';
import CopyableAddress from './CopyableAddress.vue';
import Navbar from './Navbar.vue';
import Entry from './links/Entry';
import { getBlockEntries } from './homeBlockEntries';

const components = {
    SearchField,
    CopyableAddress,
    Navbar,
    Entry
};

export default {
    name: 'home',
    created() {
        this.update();
    },
    timers: {
        ticker: { time: 2000, autostart: true, repeat: true }
    },
    data() {
        return {
            lastCommitted: 0,
            lastVerified: 0,
            totalTransactions: 0,
            currentPage: this.$route.query.page || 1,

            txPerBlock: constants.TX_BATCH_SIZE,
            blocks: [],
            ready: false,

            loading: true,
            contractAddress: null,

            breadcrumbs: [
                {
                    text: 'Blocks',
                    active: true
                }
            ],

            updateError: null,
            inactive: false
        };
    },
    computed: {
        page() {
            return this.$route.query.page || 1;
        },
        items() {
            return this.blocks;
        },
        perPage() {
            return constants.PAGE_SIZE;
        },
        rows() {
            return this.lastCommitted || 9999;
        }
    },
    activated() {
        this.inactive = false;
    },
    deactivated() {
        this.inactive = true;
    },
    methods: {
        async ticker() {
            try {
                if (!this.inactive) {
                    await this.update(true);
                }
                this.updateError = null;
            } catch (e) {
                this.updateError = e.message || 'Unknown error';
            }
        },
        onRowClicked(item) {
            this.$router.push('/blocks/' + item.block_number);
        },
        onPageChanged(page) {
            this.$router.push(`${this.$route.path}?page=${page}`);
        },
        async update(silent) {
            if (!silent) {
                this.loading = true;
            }

            const client = await clientPromise;

            const status = await client.status();

            let newBlocks = false;
            if (status) {
                newBlocks = this.lastCommitted !== status.last_committed || this.lastVerified !== status.last_verified;
                this.lastCommitted = status.last_committed;
                this.lastVerified = status.last_verified;
                this.totalTransactions = status.total_transactions;
            }

            if (newBlocks) {
                await this.updateBlocks();
            }

            this.loading = false;
        },
        async updateBlocks() {
            const client = await clientPromise;

            const max_block = this.lastCommitted - constants.PAGE_SIZE * (this.currentPage - 1);
            if (max_block < 0) return;

            const blocks = await client.loadBlocks(max_block);
            if (blocks) {
                this.blocks = blocks.map(getBlockEntries);
                this.currentPage = this.page;
                this.ready = true;
            }
            this.loading = false;
        }
    },
    watch: {
        $route() {
            if (!this.inactive) {
                this.currentPage = this.page;
                this.updateBlocks();
            }
        }
    },
    components
};
</script>

<style>
.capitalize:first-letter {
    text-transform: capitalize;
}

.table-container {
    position: relative;
}

.overlay {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
}

.clickable tr {
    cursor: pointer;
}

.num {
    font-size: 3em;
}

@media (max-width: 720px) {
    .hide-sm {
        display: none;
    }
}

@media (max-width: 992px) {
    .hide-lg {
        display: none;
    }
}

h1,
h2,
h3,
h4 {
    font-weight: bold;
}

body {
    font-size: 0.9rem;
}

.btn {
    font-size: 0.8rem;
}

.block_number_link {
    display: inline-block;
    width: 8em;
    margin-left: -1em;
    padding-left: 1em;
    /* box-shadow: inset 0 0 5px rgba(0, 0, 0, 0.1); */
    cursor: pointer;
    /* text-decoration: underline; */
}
</style>
