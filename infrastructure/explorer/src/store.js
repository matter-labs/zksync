import config from './env-config';
import { capitalize } from './utils';

const network = config.ETH_NETWORK;
const walletLinkPrefix = network == 'mainnet' ? 'wallet' : network;

const store = {
    contractAddress: undefined,
    config: network,
    network: network,
    capitalizedNetwork: capitalize(network),
    walletLink: `https://${walletLinkPrefix}.zksync.io`,
    statusLink: 'https://uptime.com/s/zksync'
};

export default store;
