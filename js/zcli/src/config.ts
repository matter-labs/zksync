import fs from 'fs';
import { Config, Wallet, ALL_NETWORKS } from './common';
import assert from 'assert';

const CONFIG_FILE = '.zcli-config.json';
const DEFAULT_CONFIG: Config = {
    network: 'ropsten',
    defaultWallet: null,
    wallets: []
};

function configLocation() {
    const in_pwd = './' + CONFIG_FILE;
    const zcli_home = process.env.ZCLI_HOME;
    if (fs.existsSync(in_pwd) || !zcli_home) {
        return in_pwd;
    }
    if (!fs.existsSync(zcli_home) || !fs.lstatSync(zcli_home).isDirectory()) {
        console.warn('$ZCLI_HOME is not pointing to a valid directory; ignoring...');
        return in_pwd;
    } else {
        return `${zcli_home}/${CONFIG_FILE}`;
    }
}

export function loadConfig(): Config {
    const config_path = configLocation();
    if (!fs.existsSync(config_path)) {
        return DEFAULT_CONFIG;
    }
    const unparsed = fs.readFileSync(config_path);
    try {
        const parsed = JSON.parse(unparsed.toString());
        assert(ALL_NETWORKS.includes(parsed.network));
        assert(Array.isArray(parsed.wallets));
        assert(
            parsed.defaultWallet === null
            || parsed.wallets
                .map((w: Wallet) => w.address)
                .includes(parsed.defaultWallet)
        );
        return parsed;
    } catch (err) {
        console.warn('Invalid .zcli-config.json; ignoring...');
        return DEFAULT_CONFIG;
    }
}

export function saveConfig(config: Config) {
    const config_path = configLocation();
    const config_string = JSON.stringify(config, null, 4);
    fs.writeFileSync(config_path, config_string);
}

