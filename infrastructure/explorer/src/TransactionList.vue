<template>
    <div>
        <b-table
            responsive
            id="my-table"
            hover
            outlined
            :items="transactions"
            :per-page="rowsPerPage"
            :current-page="currentPage"
            :fields="fields"
            class="nowrap"
        >
            <template v-slot:cell(txHash)="data">
                <Entry :value="data.item['txHash'].value" />
            </template>
            <template v-slot:cell(type)="data">
                <Entry :value="data.item['type'].value" />
            </template>
            <template v-slot:cell(from)="data">
                <Entry :value="data.item['from'].value" />
            </template>
            <template v-slot:cell(to)="data">
                <Entry :value="data.item['to'].value" />
            </template>
            <template v-slot:cell(amount)="data">
                <Entry :value="data.item['amount'].value" />
            </template>
            <template v-slot:cell(fee)="data">
                <Entry :value="data.item['fee'].value" />
            </template>
            <template v-slot:cell(createdAt)="data">
                <Entry :value="data.item['createdAt'].value" />
            </template>
        </b-table>
        <b-pagination
            v-if="transactions.length > rowsPerPage"
            class="mt-2 mb-2"
            v-model="currentPage"
            :per-page="rowsPerPage"
            :total-rows="transactions.length"
        ></b-pagination>
    </div>
</template>

<script>
import CopyableAddress from './CopyableAddress.vue';
import { shortenHash } from './utils';
import Entry from './links/Entry.vue';

const components = {
    CopyableAddress,
    Entry
};

export default {
    name: 'transaction-list',
    props: ['blockNumber', 'transactions'],
    data: () => ({
        currentPage: 1,
        rowsPerPage: 1000
    }),
    methods: {
        shortenHash
    },
    computed: {
        fields() {
            if (this.transactions.length == 0) {
                return [];
            }

            const hiddenFields = ['fromAddr', 'toAddr', 'success', 'from_explorer_link', 'to_explorer_link'];
            return Object.keys(this.transactions[0]).filter((k) => !hiddenFields.includes(k));
        }
    },
    components
};
</script>

<style></style>
