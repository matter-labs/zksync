import 'isomorphic-fetch';
import * as zksync from 'zksync';

export type Network = "localhost" | "mainnet" | "ropsten" | "rinkeby";

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
            status: tx.fail_reason ? 'failed' : 'success',
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
