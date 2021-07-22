import store from './store';
import config from './env-config';
import { Readiness } from './Readiness';
import { getDefaultProvider } from 'zksync';

export const sleep = async (ms) => await new Promise((resolve) => setTimeout(resolve, ms));

const readablyPrintableTokens = ['ETH', 'FAU'];

export function isReadablyPrintable(tokenName) {
    return readablyPrintableTokens.includes(tokenName);
}

export function removeTxHashPrefix(txHash) {
    let nonPrefixHash = txHash;
    for (const prefix of ['0x', 'sync-tx:', 'sync-bl:', 'sync:']) {
        if (nonPrefixHash.startsWith(prefix)) {
            nonPrefixHash = nonPrefixHash.slice(prefix.length);
        }
    }
    return nonPrefixHash;
}

export function shortenHash(str, fallback) {
    try {
        return `${str.slice(0, 12)}...`;
    } catch (e) {
        return fallback || 'unknown';
    }
}

export function formatDate(timeStr) {
    if (timeStr == null) {
        return '';
    }

    return timeStr.toString().split('T')[0] + ' ' + timeStr.toString().split('T')[1].slice(0, 8) + ' UTC';
}

export function formatToken(amount, token) {
    return window.syncProvider.tokenSet.formatToken(token, amount);
}

export function capitalize(s) {
    if (typeof s !== 'string') {
        return '';
    }
    if (!s) {
        return;
    }
    return s[0].toUpperCase() + s.slice(1);
}

export function isBlockVerified(block) {
    return !!block && !!block.verified_at;
}

export function getLocalAccountLink(address) {
    return `/address/${address}`;
}

export function blockchainExplorerToken(token, account) {
    if (store.network === 'localhost') {
        return `http://localhost:8000/${account}`;
    }
    const prefix = store.network === 'mainnet' ? '' : `${store.network}.`;
    const tokenAddress = window.syncProvider.tokenSet.resolveTokenAddress(token);

    if (tokenAddress != '0x0000000000000000000000000000000000000000') {
        return `https://${prefix}etherscan.io/token/${tokenAddress}?a=${account}`;
    } else {
        return `https://${prefix}etherscan.io/address/${account}`;
    }
}

export function getBlockchainExplorerTx(network) {
    if (network === 'localhost') {
        return 'http://localhost:8000';
    }
    if (network === 'mainnet') {
        return 'https://etherscan.io/tx';
    }

    return `https://${network}.etherscan.io/tx`;
}

export function getBlockchainExplorerAddress(network) {
    if (network === 'localhost') {
        return 'http://localhost:8000';
    }
    if (network === 'mainnet') {
        return 'https://etherscan.io/address';
    }

    return `https://${network}.etherscan.io/address`;
}

export function readyStateFromString(s) {
    return {
        Rejected: Readiness.Rejected,
        Initiated: Readiness.Initiated,
        Pending: Readiness.Committed,
        Complete: Readiness.Verified,
        Scheduled: Readiness.Scheduled,
        // 'Verified' is a block version of the word 'Complete'
        Verified: Readiness.Verified
    }[s];
}

export function accountStateToBalances(account) {
    let balances = Object.entries(account.committed.balances).map(([tokenSymbol, balance]) => {
        return {
            tokenSymbol,
            balance: formatToken(balance, tokenSymbol)
        };
    });

    balances.sort((a, b) => a.tokenSymbol.localeCompare(b.tokenSymbol));

    return balances;
}

function getForcedExitEndpoint(str) {
    const FORCED_EXIT_API = `${config.API_SERVER}/api/forced_exit_requests/v0.1`;
    return FORCED_EXIT_API + str;
}

async function checkEligibilty(address) {
    const endpoint = getForcedExitEndpoint(`/checks/eligibility/${address}`);

    const response = await fetch(endpoint);

    const responseObj = await response.json();

    return responseObj.eligible;
}

export async function isEligibleForForcedExit(address) {
    if (!window.provider) {
        window.provider = await getDefaultProvider(config.ETH_NETWORK);
    }

    const state = await window.provider.getState(address);

    if (!state.id || state.id === -1) {
        // The account does not exist
        return false;
    }

    if (state.committed.nonce) {
        // The account has done some txs before
        return false;
    }

    const existedForEnoughTime = await checkEligibilty(address);
    if (!existedForEnoughTime) {
        return false;
    }

    return true;
}

// Note that this class follows Builder pattern
// If you see any of it's methods not returning `this`
// it is a bug.
//
// Used to represent the "data" part of the Entry component
// It has 2 properties:
// - name. It is used mostly to aid the programmer when dealing with bootstrap tables
// - value. The data which describes the Entry and should passed to the `value` prop
//          of the Entry component
class Entry {
    constructor(name) {
        this.name = name;
        this.value = {};
    }

    // If the link does not redirect user to another page
    // this method should be called with the address relative to
    // the routerBase be passed to it.
    //
    // Note that it only sets the router-link and its address
    // but does not change the inner content
    localLink(to) {
        this.value.isLocalLink = true;
        this.value.to = to;
        return this;
    }

    // If the link redirects user to another page
    // with the target URL passed to it.
    //
    // Note that it only sets the router-link and its address
    // but does not change the inner content
    outterLink(to) {
        this.value.isOutterLink = true;
        this.value.to = to;
        return this;
    }

    // Pass here the html of the content of the entry
    // Even though we should avoid hard-coding html,
    // in rare cases it is much more convenient and readable.
    innerHTML(innerHTML) {
        this.value.innerHTML = innerHTML;
        return this;
    }

    // Used to set layer icons
    // Example:
    // https://zkscan.io/transactions/0xccacad609c8ae5703b2bb00fd277ba2e6dd6f1d888c3963b99576aca3e3fbae8
    //
    // You should pass number 1 or number 2 depending on the layer.
    layer(layer) {
        this.value.layer = layer;
        return this;
    }

    // Makes the entry "copyable". Under the current implementation
    // it means that the entry has copy icon appended to it.
    //
    // Pass the value if the component will be copyable
    copyable(newValue = true) {
        this.value.copyable = newValue;
        return this;
    }

    // Adds tooltip to the right.
    // Note that this should be used only in combination with
    // copyable()
    tooltipRight(newValue = true) {
        this.value.tooltipRight = newValue;
        return this;
    }

    // Can be used to rename the entry
    rename(newName) {
        this.name = newName;
        return this;
    }

    // Can be used to set readiness status of a transaction or block
    status(status) {
        const isValidStatus = Object.values(Readiness).includes(status);
        if (!isValidStatus) {
            throw new Error('Invalid status');
        }

        this.value.status = status;
        return this;
    }
}

export function makeEntry(name) {
    return new Entry(name);
}
