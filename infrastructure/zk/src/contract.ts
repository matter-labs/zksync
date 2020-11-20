import { Command } from 'commander';
import * as utils from './utils';
import * as env from './env';
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
        console.error('Please download the keys');
        throw err;
    }
}

export async function build() {
    await utils.confirmAction();
    prepareVerify();
    await utils.spawn('cargo run --release --bin gen_token_add_contract');
    await utils.spawn('yarn contracts build');
}

async function prepareTestContracts() {
    const inDir = 'contracts/contracts';
    const outDir = 'contracts/dev-contracts/generated';
    fs.rmSync(outDir, { recursive: true, force: true });
    fs.mkdirSync(outDir, { recursive: true });

    await Promise.all([
        fs.promises.copyFile(`${inDir}/Governance.sol`, `${outDir}/GovernanceTest.sol`),
        fs.promises.copyFile(`${inDir}/Verifier.sol`, `${outDir}/VerifierTest.sol`),
        fs.promises.copyFile(`${inDir}/ZkSync.sol`, `${outDir}/ZkSyncTest.sol`),
        fs.promises.copyFile(`${inDir}/Storage.sol`, `${outDir}/StorageTest.sol`),
        fs.promises.copyFile(`${inDir}/Config.sol`, `${outDir}/ConfigTest.sol`),
        fs.promises.copyFile(`${inDir}/UpgradeGatekeeper.sol`, `${outDir}/UpgradeGatekeeperTest.sol`),
        fs.promises.copyFile(`${inDir}/ZkSync.sol`, `${outDir}/ZkSyncTestUpgradeTarget.sol`)
    ]);

    fs.readdirSync(outDir).forEach((file) => {
        if (!file.endsWith('.sol')) return;
        utils.modifyFile(`${outDir}/${file}`, (source) =>
            source
                .replace(/Governance/g, 'GovernanceTest')
                .replace(/\bVerifier\b/g, 'VerifierTest')
                .replace(/ZkSync/g, 'ZkSyncTest')
                .replace(/Storage/g, 'StorageTest')
                .replace(/Config/g, 'ConfigTest')
                .replace(/UpgradeGatekeeper/g, 'UpgradeGatekeeperTest')
        );
    });

    const setConstant = (target: string, name: string, value: string) => {
        const regex = new RegExp(`(constant ${name} =)(.*?);`, 'gs');
        return target.replace(regex, `$1 ${value};`);
    };

    const createGetter = (target: string, name: string) => {
        const regex = new RegExp(`    (.*) constant ${name} =(.|\s)*?;.*`, 'g');
        return target.replace(
            regex,
            `    $&\n    function get_${name}() external pure returns ($1) {\n        return ${name};\n    }`
        );
    };

    utils.modifyFile(`${outDir}/ConfigTest.sol`, (config) => {
        config = setConstant(config, 'MAX_AMOUNT_OF_REGISTERED_TOKENS', '5');
        config = setConstant(config, 'EXPECT_VERIFICATION_IN', '8');
        config = setConstant(config, 'MAX_UNVERIFIED_BLOCKS', '4');
        config = setConstant(config, 'PRIORITY_EXPIRATION', '101');
        config = setConstant(config, 'UPGRADE_NOTICE_PERIOD', '4');
        config = createGetter(config, 'MAX_AMOUNT_OF_REGISTERED_TOKENS');
        config = createGetter(config, 'EXPECT_VERIFICATION_IN');
        return config;
    });

    utils.modifyFile(`${outDir}/VerifierTest.sol`, (s) => setConstant(s, 'DUMMY_VERIFIER', 'true'));
    utils.modifyFile(`${outDir}/UpgradeGatekeeperTest.sol`, (s) => createGetter(s, 'UPGRADE_NOTICE_PERIOD'));
    utils.replaceInFile(
        `${outDir}/ZkSyncTestUpgradeTarget.sol`,
        'contract ZkSyncTest',
        'contract ZkSyncTestUpgradeTarget'
    );
    utils.replaceInFile(`${outDir}/ZkSyncTestUpgradeTarget.sol`, /revert\("upgzk"\);(.*)/g, '/*revert("upgzk");*/$1');
}

export async function buildDev() {
    await utils.confirmAction();
    prepareVerify();
    await prepareTestContracts();
    await utils.spawn('yarn contracts build-dev');
}

export async function publish() {
    await utils.spawn('yarn contracts publish-sources');
}

export async function deploy() {
    await utils.confirmAction();
    console.log('Deploying contracts, results will be inserted into the db');
    await utils.spawn('yarn contracts deploy-no-build | tee deploy.log');
    const deployLog = fs.readFileSync('deploy.log').toString();
    const envVars = [
        'GOVERNANCE_TARGET_ADDR',
        'VERIFIER_TARGET_ADDR',
        'CONTRACT_TARGET_ADDR',
        'GOVERNANCE_ADDR',
        'CONTRACT_ADDR',
        'VERIFIER_ADDR',
        'GATEKEEPER_ADDR',
        'DEPLOY_FACTORY_ADDR',
        'GENESIS_TX_HASH'
    ];
    for (const envVar of envVars) {
        const pattern = new RegExp(`${envVar}=.*`, 'g');
        const matches = deployLog.match(pattern);
        if (matches !== null) {
            env.modify(envVar, matches[0]);
        }
    }
}

export async function redeploy() {
    await deploy();
    await db.insert.contract();
    await publish();
}

export const command = new Command('contract').description('contract management');

command.command('prepare-verify').description('initialize verification keys for contracts').action(prepareVerify);
command.command('redeploy').description('redeploy contracts and update addresses in the db').action(redeploy);
command.command('deploy').description('deploy contracts').action(deploy);
command.command('build').description('build contracts').action(build);
command.command('build-dev').description('build development contracts').action(buildDev);
command.command('publish').description('publish contracts').action(publish);
