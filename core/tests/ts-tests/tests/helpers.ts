import * as fs from 'fs';
import { Provider } from 'zksync';
import { sleep } from 'zksync/build/utils';

function configPath(postfix: string) {
    return `${process.env.ZKSYNC_HOME}/etc/test_config/${postfix}`;
}

function loadConfig(path: string) {
    return JSON.parse(
        fs.readFileSync(path, {
            encoding: 'utf-8'
        })
    );
}

export function loadTestConfig() {
    const eipConstantPath = configPath('constant/eip1271.json');
    const eipVolatilePath = configPath('volatile/eip1271.json');

    const eipConstantConfig = loadConfig(eipConstantPath);
    const eipVolatileConfig = loadConfig(eipVolatilePath);
    const eipConfig = Object.assign(eipConstantConfig, eipVolatileConfig);

    const ethConstantPath = configPath('constant/eth.json');
    const ethConfig = loadConfig(ethConstantPath);

    const withdrawalHelpersConfigPoth = configPath('volatile/withdrawal-helpers.json');
    const withdrawalHelpersConfig = loadConfig(withdrawalHelpersConfigPoth);

    return {
        eip1271: eipConfig,
        eth: ethConfig,
        withdrawalHelpers: withdrawalHelpersConfig
    };
}

export function loadTestVectorsConfig() {
    let vectorsConfigPath = configPath('sdk/test-vectors.json');
    return loadConfig(vectorsConfigPath);
}

export function getTokens(network: string) {
    const configPath = `${process.env.ZKSYNC_HOME}/etc/tokens/${network}.json`;
    console.log(configPath);
    return JSON.parse(
        fs.readFileSync(configPath, {
            encoding: 'utf-8'
        })
    );
}

export function getRevertReceiveContractAbi() {
    return fs.readFileSync(
        `${process.env.ZKSYNC_HOME}/contracts/`
    )
}

export async function waitForOnchainWithdrawal(
    syncProvider: Provider,
    hash: string,
    polling_interval: number = 200,
    polling_timeout: number = 35000
): Promise<string|null> {
    let withdrawalTxHash = null;
    const polling_iterations = polling_timeout / polling_interval;
    for (let i = 0; i < polling_iterations; i++) {
        withdrawalTxHash = await syncProvider.getEthTxForWithdrawal(hash);
        if (withdrawalTxHash != null) {
            break;
        }
        await sleep(polling_interval);
    }

    return withdrawalTxHash;
}
