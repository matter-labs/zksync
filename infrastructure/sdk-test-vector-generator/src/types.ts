import * as zksync from "zksync";

export interface TestVectorEntry {
    inputs: any;
    outputs: any;
}

export interface CryptoPrimitivesTestEntry extends TestVectorEntry {
    inputs: {
        // Seed to generate private key.
        seed: string;
        // Message to be signed.
        message: string;
    };
    outputs: {
        // Private key to be obtained from seed.
        privateKey: string;
        // Hash of a public key corresponding to the generated private key.
        pubKeyHash: string;
        // Signature obtained using private key and message.
        signature: string;
    };
}

export interface TxTestEntry extends TestVectorEntry {
    inputs: {
        // Type of transaction. Valid values are: `Transfer`, `Withdraw`, `ChangePubKey`, `ForcedExit`.
        type: string;
        // Ethereum private key. zkSync private key should be derived from it.
        ethPrivateKey: string;
        // Transaction-specific input.
        data: any;
        // Transactin-specific input to generate Ethereum signature.
        // Can be `null` if Ethereum signature is not required for transaction
        ethSignData: any | null;
    };
    outputs: {
        // Encoded transaction bytes to be used for signing.
        signBytes: string;
        // Transaction zkSync signature.
        signature: zksync.types.Signature;
        // Message to be used to provie Ethereum signature. `null` if `inputs.ethSignData` is `null`.
        ethSignMessage: string | null;
        // Ethereum signature for a transaction. `null` if `inputs.ethSignData` is `null`.
        ethSignature: string | null;
    };
}

export interface TestVector<T> {
    description: string;
    items: T[];
}
