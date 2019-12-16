const Axios = require('axios');

export class BlockExplorerClient {
    constructor(providerAddress) {
        this.providerAddress = providerAddress;
    }

    async getAccountTransactions(address, offset, limit) {
        address = address.replace('sync:', '0x');

        const transactionsUrl = `${this.providerAddress}/api/v0.1/account/${address}/history/${offset}/${limit}`;
        return await Axios
            .get(transactionsUrl)
            .then(res => res.data);
    }
}
