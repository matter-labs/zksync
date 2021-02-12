/** Stores the next nonce with which the transaction will be sent. */
export class EthParameters {
    constructor(private nonce: number) {}

    public getNextNonce() {
        const currentNonce = this.nonce;
        this.nonce++;

        return currentNonce;
    }
}
