<template>
    <div>
        <b-form @submit.stop.prevent="search">
            <b-input-group>
                <b-form-input v-model="query" placeholder="block number, tx hash, state root hash, account address"></b-form-input>
                <b-input-group-append>
                <b-button @click="search" variant="info" :disabled="searching">
                    <b-spinner v-if="searching" small></b-spinner>
                    <span>Search</span>
                </b-button>
                </b-input-group-append>
                <b-form-invalid-feedback v-if="notFound" :state="false">
                    Nothing found for query '{{query}}'.
                </b-form-invalid-feedback>
            </b-input-group>
        </b-form>
    </div>
</template>

<script>
import client from './client';

export default {
    name: 'SearchField',
    data: () => ({
        query:              null,
        loading:            true,
        searching:          false,
        notFound:           false,
    }),
    methods: {
        async search() {
            if (this.query == null) return;

            this.notFound = false;
            this.searching = true;

            let query = this.query.trim();

            let block = await client.searchBlock(query);
            if (block && block.block_number) {
                this.$router.push('/blocks/' + block.block_number);
                this.searching = false;
                return;
            }

            let tx = await client.searchTx(query);
            if (tx && tx.tx_type) {
                this.$router.push('/transactions/' + query);
                this.searching = false;
                return;
            }

            let account = await client.searchAccount(query);
            if (account && account.id) {
                this.$router.push('/accounts/' + query);
                this.searching = false;
                return;
            }

            this.searching = false;
            this.notFound = true;
            await new Promise(resolve => setTimeout(resolve, 3600));
            this.notFound = false;
        },
    },
}
</script>
