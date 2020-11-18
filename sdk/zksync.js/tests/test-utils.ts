import * as fs from 'fs';

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
    let ethConfig = loadConfig(ethConstantPath);

    return {
        eip1271: eipConfig,
        eth: ethConfig
    };
}
