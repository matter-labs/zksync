import 'isomorphic-fetch';
import * as zksync from 'zksync';
import { Wallet as EthWallet } from 'ethers';
import { saveConfig } from './config';
import { ALL_NETWORKS, Network, Wallet, Config, AccountInfo, TxInfo } from './common';

async function tokenInfo(id: number, provider: zksync.Provider) {
    const tokens = await provider.getTokens();
    const tokenInfo = Object.values(tokens).find(value => value.id == id);
    return tokenInfo;
}

export async function accountInfo(address: string, network: Network): Promise<AccountInfo> {
    const provider = await zksync.getDefaultProvider(network);
    const state = await provider.getState(address);
    let balances: { [token: string]: string } = {};
    for (const token in state.verified.balances) {
        balances[token] = provider.tokenSet.formatToken(token, state.verified.balances[token]);
    }
    await provider.disconnect();
    return {
        address,
        network,
        account_id: state.id,
        nonce: state.verified.nonce,
        balances
    };
}

export async function txInfo(tx_hash: string, network: Network): Promise<TxInfo> {
    const subdomain = network === 'mainnet' ? 'api' : `${network}-api`
    const api_url = `https://${subdomain}.zksync.io/api/v0.1/transactions_all/${tx_hash}`;
    const response = await fetch(api_url);
    const tx = await response.json();
    if (tx === null) {
        return {
            network,
            transaction: null
        };
    }
    const provider = await zksync.getDefaultProvider(network);
    const token = await tokenInfo(tx.token, provider);
    await provider.disconnect();
    const tokenSymbol = token?.symbol as string;
    return {
        network,
        transaction: {
            status: tx.fail_reason ? 'error' : 'success',
            from: tx.from,
            to: tx.to,
            hash: tx_hash,
            operation: tx.tx_type.toLowerCase(),
            token: tokenSymbol,
            amount: provider.tokenSet.formatToken(tokenSymbol, tx.amount),
            fee: provider.tokenSet.formatToken(tokenSymbol, tx.fee),
            nonce: tx.nonce
        }
    };
}

export async function availableNetworks() {
    let networks: Network[] = [];
    for (const network of ALL_NETWORKS) {
        try {
            const provider = await zksync.getDefaultProvider(network);
            provider.disconnect();
            networks.push(network);
        } catch (err) { /* could not connect to provider */ }
    }
    return networks;
}

export function defaultNetwork(config: Config, network?: Network) {
    if (network) {
        if (ALL_NETWORKS.includes(network)) {
            config.network = network;
            saveConfig(config);
        } else {
            throw Error('invalid network name');
        }
    }
}

export function addWallet(config: Config, privkey?: string) {
    const wallet = privkey ? new EthWallet(privkey) : EthWallet.createRandom();
    const address = wallet.address.toLowerCase();
    config.wallets.push({
        address,
        privkey: wallet.privateKey
    });
    if (!config.defaultWallet) {
        config.defaultWallet = address;
    }
    saveConfig(config);
    return wallet.address;
}

export function listWallets(config: Config) {
    let wallets: string[] = [];
    for (const { address } of config.wallets) {
        wallets.push(address);
    }
    return wallets;
}

export function removeWallet(config: Config, address: string) {
    address = address.toLowerCase();
    config.wallets = config.wallets
        .filter((w: Wallet) => w.address != address);
    if (config.defaultWallet === address) {
        config.defaultWallet = null;
    }
    saveConfig(config);
}

export function defaultWallet(config: Config, address?: string) {
    if (address) {
        address = address.toLowerCase();
        const addresses = config.wallets
            .map((w: Wallet) => w.address);
        if (addresses.includes(address)) {
            config.defaultWallet = address;
            saveConfig(config);
        } else {
            throw Error('address is not present');
        }
    }
}
