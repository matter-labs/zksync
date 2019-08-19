import axios from 'axios'
import store from './store'

async function fetch(req) {
    let r = await axios(req)
    if (r.status == 200) {
        return r.data
    } else {
        return null
    }
}

function baseUrl() {
    return store.config.API_SERVER + '/api/v0.1' //'http://localhost:3000/api/v0.1'
}

let self = {
    
    PAGE_SIZE:      20, // blocks per page

    TX_PER_BLOCK() {
        return store.config.TX_BATCH_SIZE 
    },
    
    async status() {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/status`,
        })
    },

    async loadBlocks(max) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks?max_block=${max}&limit=${self.PAGE_SIZE}`,
        })
    },

    async getBlock(blockNumber) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks/${blockNumber}`,
        })
    },

    async getBlockTransactions(blockNumber) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks/${blockNumber}/transactions`,
        })
    },

    async searchBlock(query) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/search?query=${query}`,
        })
    },
}

window.client = self

export default self