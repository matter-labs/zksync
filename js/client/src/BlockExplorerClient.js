const Axios = require('axios');

export class BlockExplorerClient {
    constructor(providerAddress) {
        this.providerAddress = providerAddress;
    }
    async getAccountTransactions(address, offset, limit) {
        const transactionsUrl = `${this.providerAddress}/api/v0.1/account/${address}/history/${offset}/${limit}`;
        return await Axios
            .get(transactionsUrl)
            .then(res => res.data);
    }

    async getTxReceipt(tx_hash) {
        return await Axios
            .get(this.providerAddress + '/api/v0.1/transactions/' + tx_hash)
            .then(res => res.data);
    }

    //* TODO: can we use ZKSync instead?
    async getPriorityOpReceipt(pq_id) {
        return await Axios
            .get(`${this.providerAddress}/api/v0.1/priority_operations/${pq_id}/`)
            .then(res => res.data);
    }
}
