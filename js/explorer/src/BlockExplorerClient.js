import Axios from 'axios';

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
}
