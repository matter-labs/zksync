import axios from 'axios'

const BASE_URL      = 'http://localhost:3000/api/v0.1'
const LIMIT         = 50 // blocks per page
const TX_PER_BLOCK  = 256

export default {
    
    TX_PER_BLOCK,

    async status() {
        let r = await axios({
            method:     'get',
            url:        `${BASE_URL}/status`,
        })
        if (r.status == 200) {
            return r.data
        } else {
            return null
        }
    },

    async loadBlocks(max) {
        let r = await axios({
            method:     'get',
            url:        `${BASE_URL}/blocks?max=${max}&limit=${LIMIT}`,
        })
        if (r.status == 200) {
            return r.data
        } else {
            return null
        }
    },

}