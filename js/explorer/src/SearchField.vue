<template>
    <div :class="{container: searchFieldInMenu}">
        <b-form @submit.stop.prevent="search">
            <b-input-group position="relative">
                <b-form-input v-model="query" placeholder="block number, tx hash, state root hash, account address"></b-form-input>
                <b-input-group-append>
                <b-button @click="search" :variant="searchFieldInMenu ? 'info' : 'info'" style="box-shadow: inset 0 0 2px rgba(255, 255, 255, 0.4);" :disabled="searching">
                    <b-spinner v-if="searching" small></b-spinner>
                    <span>Search</span>
                </b-button>
                </b-input-group-append>
                <b-form-invalid-feedback v-if="notFound && !searchFieldInMenu" :state="false">
                    Nothing found for query '{{query}}'.
                </b-form-invalid-feedback>
                <b-form-invalid-feedback v-if="notFound && searchFieldInMenu" class="search-field-in-menu" :state="false">
                    Nothing found.
                </b-form-invalid-feedback>
            </b-input-group>
        </b-form>
    </div>
</template>

<script>
import { clientPromise } from './Client';

export default {
    name: 'SearchField',
    props: ['searchFieldInMenu'],
    data: () => ({
        query:              null,
        loading:            true,
        searching:          false,
        notFound:           false,
    }),
    methods: {
        async search() {
            if (this.query == null) return;

            const client = await clientPromise;

            this.notFound = false;
            this.searching = true;

            let query = this.query.trim();
            for (const prefix of ['0x', 'sync-tx:', 'sync-bl:', 'sync:'])
                if (query.startsWith(prefix)) 
                    query = query.slice(prefix.length);

            let block = await client.searchBlock(query).catch(() => null);
            if (block && block.block_number) {
                this.$router.push('/blocks/' + block.block_number);
                this.searching = false;
                return;
            }

            let tx = await client.searchTx(query).catch(() => null);
            if (tx && tx.tx_type) {
                const prefix = '';
                this.$router.push('/transactions/' + prefix + query);
                this.searching = false;
                return;
            }

            let account = await client.getAccount('0x' + query).catch(() => null);
            if (account && account.id) {
                this.$router.push('/accounts/0x' + query);
                this.searching = false;
                return;
            }

            this.searching = false;
            this.notFound = true;
            await new Promise(resolve => setTimeout(resolve, 3600));
            this.notFound = false;
        },
    },
};
</script>

<style scoped>
.search-field-in-menu {
    position: absolute; 
    top: 3.2em; 
    background: #eee;  
    line-height: 2.2;
    padding-left: 0.4em;
    background: #eee;
    text-align: center;
    border-radius: 3px;
    white-space: nowrap;
}

@media (min-width: 1000px) {
    .container {
        width: 32em;
    }
}
@media (min-width: 1200px) {
    .container {
        width: 40em;
    }
}
</style>
 