import 'isomorphic-fetch';
import * as zksync from 'zksync';
import { Wallet as EthWallet } from 'ethers';
import { saveConfig } from './config';
import { ALL_NETWORKS, Network, Wallet, Config } from './common';

async function tokenInfo(id: number, provider: zksync.Provider) {
    const tokens = await provider.getTokens();
    const tokenInfo = Object.values(tokens).find(value => value.id == id);
    return tokenInfo;
}

export async function accountInfo(address: string, network: Network) {
    const provider = await zksync.getDefaultProvider(network);
    const state = await provider.getState(address);
    const balances = state.verified.balances;
    for (const token in balances) {
        balances[token] = provider.tokenSet.formatToken(token, balances[token]);
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

export async function txInfo(tx_hash: string, network: Network) {
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
            status: tx.fail_reason ? 'fail' : 'success',
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

export function addWallet(config: Config, privkey?: string) {
    const wallet = privkey ? new EthWallet(privkey) : EthWallet.createRandom();
    config.wallets.push({
        privkey: wallet.privateKey,
        address: wallet.address
    });
    if (!config.defaultWallet) {
        config.defaultWallet = wallet.address;
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
    const address_lower = address.toLowerCase();
    config.wallets = config.wallets
        .filter((w: Wallet) => w.address.toLowerCase() != address_lower);
    if (config.defaultWallet?.toLowerCase() === address_lower) {
        config.defaultWallet = null;
    }
    saveConfig(config);
}

