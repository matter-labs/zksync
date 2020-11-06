import * as fs from 'fs';
import { Network, Config } from './types';

const CONFIG_FILE = '.analytics-config.json';

function configPath() {
    const env_directory = process.env.ANALYTICS_HOME;

    const cur_path = './' + CONFIG_FILE;
    const env_path = `${env_directory}/${CONFIG_FILE}`;

    if (fs.existsSync(cur_path)) {
        return cur_path;
    }

    if (env_directory && fs.existsSync(env_path)) {
        return env_path;
    }

    return;
}

export function loadConfig(network?: Network) {
    const config_path = configPath();

    if (!fs.existsSync(config_path)) {
        console.warn('Configuration file not found');
        return;
    }

    try {
        const config_json = fs.readFileSync(config_path);
        const parsed = JSON.parse(config_json.toString());

        if (!network) network = parsed['defaultNetwork'] as Network;

        const network_config = parsed['network'][network];

        const config: Config = {
            network: network,
            rest_api_address: network_config['REST_API_ADDR'],
            operator_fee_address: network_config['OPERATOR_FEE_ETH_ADDRESS'],
            etherscan_api_key: process.env['ETHERSCAN_API_KEY']
        };

        return config;
    } catch (err) {
        console.warn('Invalid Configuration file');
        return;
    }
}
