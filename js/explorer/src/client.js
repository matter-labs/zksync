import axios from 'axios';
import store from './store';

async function fetch(req) {
    let r = await axios(req).catch(_ => ({}));
    if (r.status == 200) {
        return r.data;
    } else {
        return null;
    }
}

function baseUrl() {
    return store.config.API_SERVER + '/api/v0.1'; //'http://localhost:3000/api/v0.1'
}

let self = {
    
    PAGE_SIZE:      20, // blocks per page

    TX_PER_BLOCK() {
        return store.config.TX_BATCH_SIZE; 
    },
    
    async status() {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/status`,
        });
    },

    async loadBlocks(max) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks?max_block=${max}&limit=${self.PAGE_SIZE}`,
        });
    },

    async getBlock(blockNumber) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks/${blockNumber}`,
        });
    },

    async getBlockTransactions(blockNumber) {
        let txs = await fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks/${blockNumber}/transactions`,
        });
        
        return txs.map(tx => {
            let res = tx.op;
            res.tx_hash = tx.tx_hash;
            return res;
        });
    },

    async getTokens() {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/tokens`,
        });
    },

    searchBlock(query) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/search?query=${query}`,
        });
    },

    searchAccount(address) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/account/${address}`,
        });
    },
    
    searchTx(tx_hash) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/transactions_all/${tx_hash}`,
        });
    },
};

window.client = self;

export default self;
