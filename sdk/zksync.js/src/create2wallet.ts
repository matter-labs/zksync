import { ethers } from "ethers";
import { Create2WalletData } from "./types";
import { calculateCreate2WalletAddressAndSalt } from "./utils";

export class Create2WalletSigner extends ethers.Signer {
    public readonly address: string;
    // salt for create2 function call
    public readonly salt: string;
    constructor(
        public zkSyncPubkeyHash: string,
        public create2WalletData: Create2WalletData,
        provider?: ethers.providers.Provider
    ) {
        super();
        Object.defineProperty(this, "provider", {
            enumerable: true,
            value: provider,
            writable: false
        });
        const create2Info = calculateCreate2WalletAddressAndSalt(zkSyncPubkeyHash, create2WalletData);
        this.address = create2Info.address;
        this.salt = create2Info.salt;
    }

    async getAddress() {
        return this.address;
    }

    /**
     * This signer can't sign messages but we return zeroed signature bytes to comply with zksync API for now.
     */
    async signMessage(_message) {
        return "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    }

    async signTransaction(_message): Promise<string> {
        throw new Error("Create2Wallet signer can't sign transactions");
    }

    connect(provider: ethers.providers.Provider): ethers.Signer {
        return new Create2WalletSigner(this.zkSyncPubkeyHash, this.create2WalletData, provider);
    }
}
