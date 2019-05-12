import axios from 'axios'

let self = {
    
    BASE_URL:       'http://localhost:3000/api/v0.1',
    PAGE_SIZE:      20, // blocks per page
    TX_PER_BLOCK:   256,
    
    async status() {
        let r = await axios({
            method:     'get',
            url:        `${self.BASE_URL}/status`,
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
            url:        `${self.BASE_URL}/blocks?max_block=${max}&limit=${self.PAGE_SIZE}`,
        })
        if (r.status == 200) {
            return r.data
        } else {
            return null
        }
    },

}

export default self