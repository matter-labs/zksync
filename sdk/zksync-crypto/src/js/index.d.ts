export function zksync_crypto_init(): void;
export function privateKeyFromSeed(seed: Uint8Array): Uint8Array;
export function private_key_to_pubkey_hash(private_key: Uint8Array): Uint8Array;
export function private_key_to_pubkey(private_key: Uint8Array): Uint8Array;
export function sign_musig(private_key: Uint8Array, msg: Uint8Array): Uint8Array;

export function isReady (): boolean;
export function waitReady (): Promise<boolean>;