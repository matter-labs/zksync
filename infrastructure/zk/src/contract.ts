import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';

import * as db from './db/db';

export function prepareVerify() {
    const keyDir = process.env.KEY_DIR;
    const accountTreeDepth = process.env.ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.BALANCE_TREE_DEPTH;
    const source = `${keyDir}/account-${accountTreeDepth}_balance-${balanceTreeDepth}/KeysWithPlonkVerifier.sol`;
    const dest = 'contracts/contracts/KeysWithPlonkVerifier.sol';
    try {
        fs.copyFileSync(source, dest);
    } catch (err) {
        console.error("Please download the keys");
        throw err;
    }
}

export async function build() {
    prepareVerify();
    await utils.spawn('cargo run --release --bin gen_token_add_contract');
    await utils.spawn('yarn --cwd contracts build')
}

async function prepareTestContracts() {
    const inDir = 'contracts/contracts';
    const outDir = 'contracts/dev-contracts/generated';
    fs.rmdirSync(outDir, { recursive: true });
    fs.mkdirSync(outDir, { recursive: true });

    fs.copyFileSync(`${inDir}/Governance.sol`, `${outDir}/GovernanceTest.sol`);
    fs.copyFileSync(`${inDir}/Verifier.sol`, `${outDir}/VerifierTest.sol`);
    fs.copyFileSync(`${inDir}/ZkSync.sol`, `${outDir}/ZkSyncTest.sol`);
    fs.copyFileSync(`${inDir}/Storage.sol`, `${outDir}/StorageTest.sol`);
    fs.copyFileSync(`${inDir}/Config.sol`, `${outDir}/ConfigTest.sol`);
    fs.copyFileSync(`${inDir}/UpgradeGatekeeper.sol`, `${outDir}/UpgradeGatekeeperTest.sol`);
    fs.copyFileSync(`${inDir}/ZkSync.sol`, `${outDir}/ZkSyncTestUpgradeTarget.sol`);

    fs.readdirSync(outDir).forEach(file => {
        if (!file.endsWith('.sol')) return;
        const source = fs.readFileSync(`${outDir}/${file}`)
            .toString()
            .replace(/Governance/g, 'GovernanceTest')
            .replace(/\bVerifier\b/g, 'VerifierTest')
            .replace(/ZkSync/g, 'ZkSyncTest')
            .replace(/Storage/g, 'StorageTest')
            .replace(/Config/g, 'ConfigTest')
            .replace(/UpgradeGatekeeper/g, 'UpgradeGatekeeperTest')
        fs.writeFileSync(`${outDir}/${file}`, source);
    });

    const source = fs.readFileSync(`${outDir}/ZkSyncTestUpgradeTarget.sol`).toString()
        .replace(/contract ZkSyncTest/g, 'contract ZkSyncTestUpgradeTarget')
    fs.writeFileSync(`${outDir}/ZkSyncTestUpgradeTarget.sol`, source);

    const setConstant = (target: string, name: string, value: string) => {
        const regex = new RegExp(`(.*constant ${name} =)(.*);`, 'g');
        return target.replace(regex, `$1 ${value};`); 
    }

    const createGetter = (target: string, name: string) => {
        const regex = new RegExp(`    (.*) (constant ${name} =)(.*);(.*)`, 'g');
        return target.replace(regex, `    $1 $2$3;$4\n    function get_${name}() external pure returns ($1) {\n        return ${name};\n    }`);
    }

    let config = fs.readFileSync(`${outDir}/ConfigTest.sol`).toString();
    config = setConstant(config, 'MAX_AMOUNT_OF_REGISTERED_TOKENS', '5');
    config = setConstant(config, 'EXPECT_VERIFICATION_IN', '8');
    config = setConstant(config, 'MAX_UNVERIFIED_BLOCKS', '4');
    config = setConstant(config, 'PRIORITY_EXPIRATION', '101');
    config = setConstant(config, 'UPGRADE_NOTICE_PERIOD', '4');
    config = createGetter(config, 'MAX_AMOUNT_OF_REGISTERED_TOKENS');
    config = createGetter(config, 'EXPECT_VERIFICATION_IN');
    fs.writeFileSync(`${outDir}/ConfigTest.sol`, config);

    const verifier = fs.readFileSync(`${outDir}/VerifierTest.sol`).toString();
    fs.writeFileSync(`${outDir}/VerifierTest.sol`, setConstant(verifier, 'DUMMY_VERIFIER', 'true'));

    const gatekeeper = fs.readFileSync(`${outDir}/UpgradeGatekeeperTest.sol`).toString();
    fs.writeFileSync(`${outDir}/UpgradeGatekeeperTest.sol`, createGetter(gatekeeper, 'UPGRADE_NOTICE_PERIOD'));

    const zksync = fs.readFileSync(`${outDir}/ZkSyncTestUpgradeTarget.sol`).toString()
        .replace(/revert\("upgzk"\);(.*)/g, '/*revert("upgzk");*/$1');
    fs.writeFileSync(`${outDir}/ZkSyncTestUpgradeTarget.sol`, zksync);
}

export async function buildDev() {
    await prepareTestContracts();
    await utils.spawn('yarn --cwd contracts build-dev');
}

export async function publish() {
    await utils.spawn('yarn --cwd contracts publish-sources');
}

export async function deploy() {
    console.log('Redeploying contracts, results will be inserted into the db');
    await utils.spawn('yarn --cwd contracts deploy-no-build | tee deploy.log');
    const deployLog = fs.readFileSync('deploy.log').toString();
    const envVars = [
        "GOVERNANCE_TARGET_ADDR",
        "VERIFIER_TARGET_ADDR",
        "CONTRACT_TARGET_ADDR",
        "GOVERNANCE_ADDR",
        "CONTRACT_ADDR",
        "VERIFIER_ADDR",
        "GATEKEEPER_ADDR",
        "DEPLOY_FACTORY_ADDR",
        "GENESIS_TX_HASH"
    ];
    for (const envVar of envVars) {
        const pattern = new RegExp(`${envVar}=.*`, 'g');
        // @ts-ignore
        utils.modifyEnv(envVar, deployLog.match(pattern)[0]);
    }
}

export async function redeploy() {
    await deploy();
    await db.insert.contract();
    await publish();
}

export const command = new Command('contract')
    .description('contract management');

command
    .command('prepare-verify')
    .description('initialize verification keys for contracts')
    .action(prepareVerify);

command
    .command('redeploy')
    .description('redeploy contracts and update addresses in the db')
    .action(redeploy);

command
    .command('deploy')
    .description('deploy contracts')
    .action(deploy);

command
    .command('build')
    .description('build contracts')
    .action(build);

command
    .command('build-dev')
    .description('build development contracts')
    .action(buildDev);

command
    .command('publish')
    .description('publish contracts')
    .action(publish);
