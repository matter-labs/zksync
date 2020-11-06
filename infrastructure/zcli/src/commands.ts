import 'isomorphic-fetch';
import * as zksync from 'zksync';
import * as ethers from 'ethers';
import { saveConfig } from './config';
import { ALL_NETWORKS, Network, Config, AccountInfo, TxInfo, TxDetails } from './types';

export function apiServer(network: Network) {
    const servers = {
        localhost: 'http://localhost:3001',
        ropsten: 'https://ropsten-api.zksync.io',
        rinkeby: 'https://rinkeby-api.zksync.io',
        mainnet: 'https://api.zksync.io'
    };
    return `${servers[network]}/api/v0.1`;
}

export async function accountInfo(address: string, network: Network = 'localhost'): Promise<AccountInfo> {
    const provider = await zksync.getDefaultProvider(network, 'HTTP');
    const state = await provider.getState(address);
    let balances: { [token: string]: string } = {};
    for (const token in state.committed.balances) {
        balances[token] = provider.tokenSet.formatToken(token, state.committed.balances[token]);
    }
    await provider.disconnect();
    return {
        address,
        network,
        account_id: state.id,
        nonce: state.committed.nonce,
        balances
    };
}

export async function txInfo(
    tx_hash: string,
    network: Network = 'localhost',
    wait: '' | 'COMMIT' | 'VERIFY' = ''
): Promise<TxInfo> {
    const provider = await zksync.getDefaultProvider(network, 'HTTP');
    if (wait !== '') {
        await provider.notifyTransaction(tx_hash, wait);
    }
    const api_url = `${apiServer(network)}/transactions_all/${tx_hash}`;
    const response = await fetch(api_url);
    const tx = await response.json();
    if (tx === null) {
        await provider.disconnect();
        return {
            network,
            transaction: null
        };
    }
    let info: TxInfo = {
        network,
        transaction: {
            status: tx.fail_reason ? 'error' : 'success',
            from: tx.from,
            to: tx.to,
            hash: tx_hash,
            operation: tx.tx_type,
            nonce: tx.nonce
        }
    };
    if (tx.token === -1) {
        await provider.disconnect();
        return info;
    }
    const tokens = await provider.getTokens();
    await provider.disconnect();
    const tokenInfo = Object.values(tokens).find((value) => value.id == tx.token);
    if (tokenInfo) {
        const token = tokenInfo.symbol; // @ts-ignore
        info.transaction.amount =
            tx.amount == 'unknown amount' ? null : provider.tokenSet.formatToken(token, tx.amount);
        if (tx.fee) {
            // @ts-ignore
            info.transaction.fee = provider.tokenSet.formatToken(token, tx.fee);
        } // @ts-ignore
        info.transaction.token = token;
    } else {
        throw new Error('token not found');
    }
    return info;
}

export async function availableNetworks() {
    let networks: Network[] = [];
    for (const network of ALL_NETWORKS) {
        try {
            const provider = await zksync.getDefaultProvider(network, 'HTTP');
            await provider.disconnect();
            networks.push(network);
        } catch (err) {
            /* could not connect to provider */
        }
    }
    return networks;
}

export function defaultNetwork(config: Config, network?: Network) {
    if (network) {
        if (ALL_NETWORKS.includes(network)) {
            config.network = network;
            saveConfig(config);
        } else {
            throw new Error('invalid network name');
        }
    }
    return config.network;
}

export function addWallet(config: Config, privkey?: string) {
    const wallet = privkey ? new ethers.Wallet(privkey) : ethers.Wallet.createRandom();
    const address = wallet.address.toLowerCase();
    config.wallets[address] = wallet.privateKey;
    if (!config.defaultWallet) {
        config.defaultWallet = address;
    }
    saveConfig(config);
    return wallet.address;
}

export function listWallets(config: Config) {
    return Object.keys(config.wallets);
}

export function removeWallet(config: Config, address: string) {
    address = address.toLowerCase();
    delete config.wallets[address];
    if (config.defaultWallet === address) {
        config.defaultWallet = null;
    }
    saveConfig(config);
}

export function defaultWallet(config: Config, address?: string) {
    if (address) {
        address = address.toLowerCase();
        if (config.wallets.hasOwnProperty(address)) {
            config.defaultWallet = address;
            saveConfig(config);
        } else {
            throw new Error('address is not present');
        }
    }
    return config.defaultWallet;
}

class TxSubmitter {
    private constructor(private syncProvider: zksync.Provider, private syncWallet: zksync.Wallet) {}

    static async submit(
        type: 'deposit' | 'transfer',
        txDetails: TxDetails,
        fast: boolean = false,
        network: Network = 'localhost'
    ) {
        const ethProvider =
            network == 'localhost' ? new ethers.providers.JsonRpcProvider() : ethers.getDefaultProvider(network);
        const syncProvider = await zksync.getDefaultProvider(network, 'HTTP');
        const ethWallet = new ethers.Wallet(txDetails.privkey).connect(ethProvider);
        const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);
        const submitter = new TxSubmitter(syncProvider, syncWallet);
        const hash = await submitter[type](txDetails, fast);
        await submitter.syncProvider.disconnect();
        return hash;
    }

    private async transfer(txDetails: TxDetails, fast: boolean) {
        const { to, token, amount } = txDetails;
        if (!(await this.syncWallet.isSigningKeySet())) {
            const changePubkey = await this.syncWallet.setSigningKey({
                feeToken: token
            });
            await changePubkey.awaitReceipt();
        }
        const txHandle = await this.syncWallet.syncTransfer({
            to,
            token,
            amount: this.syncProvider.tokenSet.parseToken(token, amount)
        });
        if (!fast) await txHandle.awaitReceipt();
        return txHandle.txHash;
    }

    private async deposit(txDetails: TxDetails, fast: boolean) {
        const { to: depositTo, token, amount } = txDetails;
        const depositHandle = await this.syncWallet.depositToSyncFromEthereum({
            depositTo,
            token,
            amount: this.syncProvider.tokenSet.parseToken(token, amount),
            approveDepositAmountForERC20: !zksync.utils.isTokenETH(token)
        });
        if (!fast) await depositHandle.awaitReceipt();
        return depositHandle.ethTx.hash;
    }
}

export const submitTx = TxSubmitter.submit;
export const deposit = async (details: TxDetails, fast: boolean = false, network: Network = 'localhost') =>
    await submitTx('deposit', details, fast, network);
export const transfer = async (details: TxDetails, fast: boolean = false, network: Network = 'localhost') =>
    await submitTx('transfer', details, fast, network);
