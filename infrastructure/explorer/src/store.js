import config from './env-config';
import { capitalize } from './utils';

const network = config.ETH_NETWORK;
const walletLink = config.WALLET_ADDRESS;
const explorerLink = config.EXPLORER;
const explorerVersion = config.EXPLORER_VERSION;

const store = {
    contractAddress: undefined,
    config: network,
    network: network,
    capitalizedNetwork: capitalize(network),
    walletLink: walletLink,
    explorerLink: explorerLink,
    explorerVersion: explorerVersion
};

export default store;
