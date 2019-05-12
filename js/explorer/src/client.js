import axios from 'axios'

async function fetch(req) {
    let r = await axios(req)
    if (r.status == 200) {
        return r.data
    } else {
        return null
    }
}
 
let self = {
    
    BASE_URL:       'http://localhost:3000/api/v0.1',
    PAGE_SIZE:      20, // blocks per page
    TX_PER_BLOCK:   256,
    
    async status() {
        return fetch({
            method:     'get',
            url:        `${self.BASE_URL}/status`,
        })
    },

    async loadBlocks(max) {
        return fetch({
            method:     'get',
            url:        `${self.BASE_URL}/blocks?max_block=${max}&limit=${self.PAGE_SIZE}`,
        })
    },

    async getBlock(number) {
        return fetch({
            method:     'get',
            url:        `${self.BASE_URL}/blocks/${number}`,
        })
    },

}

export default self