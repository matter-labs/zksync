import * as fs from 'fs';
import * as path from 'path';
import * as handlebars from 'handlebars';
import * as zksync from 'zksync';
import * as ethers from 'ethers';

function getDirPath() {
    return path.join(process.env['ZKSYNC_HOME'] as string, 'core/tests/apiary');
}

export function pasteAllFilesInOne() {
    let template = handlebars.compile(fs.readFileSync(path.join(getDirPath(), 'template.apib'), 'utf-8'), {
        noEscape: true
    });

    let replaceObject: any = {};

    const groupsFiles = fs.readdirSync(path.join(getDirPath(), 'blueprint/groups'));
    for (let file of groupsFiles) {
        const data = fs.readFileSync(path.join(getDirPath(), 'blueprint/groups', file), 'utf-8');
        replaceObject[file.replace('.apib', '') + 'Endpoints'] = data;
    }

    const typesFiles = fs.readdirSync(path.join(getDirPath(), 'blueprint/types'));
    for (const file of typesFiles) {
        const data = fs.readFileSync(path.join(getDirPath(), 'blueprint/types', file), 'utf-8');
        replaceObject[file.replace('.apib', '') + 'Types'] = data;
    }

    return template(replaceObject);
}

export async function compileCommon() {
    const data = pasteAllFilesInOne();
    let template = handlebars.compile(data, { noEscape: true });

    let replaceObject: any = await getHashesAndSignatures();
    replaceObject['isResultNullable'] = '{{isResultNullable}}';

    return template(replaceObject);
}

export async function setupWallet() {
    const ethTestConfig = JSON.parse(
        fs.readFileSync(path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant/eth.json`), {
            encoding: 'utf-8'
        })
    );
    let web3Url = (process.env.ETH_CLIENT_WEB3_URL as string).split(',')[0];
    const ethProvider = new ethers.providers.JsonRpcProvider(web3Url);
    ethProvider.pollingInterval = 100;
    const syncProvider = await zksync.getDefaultRestProvider('localhost');
    const ethWallet = ethers.Wallet.fromMnemonic(ethTestConfig.test_mnemonic as string, "m/44'/60'/0'/0/0").connect(
        ethProvider
    );

    const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);

    const depositHandle = await syncWallet.depositToSyncFromEthereum({
        depositTo: syncWallet.address(),
        token: 'ETH',
        amount: syncWallet.provider.tokenSet.parseToken('ETH', '1000')
    });
    await depositHandle.awaitReceipt();

    if (!syncWallet.isSigningKeySet()) {
        const changePubkeyHandle = await syncWallet.setSigningKey({
            feeToken: 'ETH',
            ethAuthType: 'ECDSA'
        });
        await changePubkeyHandle.awaitReceipt();
    }

    return syncWallet;
}

export async function getHashesAndSignatures() {
    let result: any = {};
    let syncWallet = await setupWallet();

    const handle = await syncWallet.syncTransfer({ to: syncWallet.address(), token: 'ETH', amount: 0 });
    await handle.awaitReceipt();
    result['txHash'] = handle.txHash;

    const batch = await syncWallet
        .batchBuilder()
        .addTransfer({ to: syncWallet.address(), token: 'ETH', amount: 0 })
        .build('ETH');
    let txs = [];
    for (const signedTx of batch.txs) {
        txs.push(signedTx.tx);
    }

    const submitBatchResponse = await (syncWallet.provider as zksync.RestProvider).submitTxsBatchNew(
        txs,
        batch.signature
    );
    await (syncWallet.provider as zksync.RestProvider).notifyAnyTransaction(
        submitBatchResponse.transactionHashes[0],
        'COMMIT'
    );
    result['txBatchHash'] = submitBatchResponse.batchHash;

    return result;
}

export async function compileForDocumentation() {
    const before = await compileCommon();
    let template = handlebars.compile(before, { noEscape: true });

    let replaceObject: any = {};
    replaceObject['isResultNullable'] = ', nullable';

    const after = template(replaceObject);

    fs.writeFileSync(path.join(getDirPath(), 'documentation.apib'), after);
}

export async function compileForTest() {
    const before = await compileCommon();
    let template = handlebars.compile(before, { noEscape: true });

    let replaceObject: any = {};
    replaceObject['isResultNullable'] = '';

    const after = template(replaceObject);

    fs.writeFileSync(path.join(getDirPath(), 'test.apib'), after);
}

compileForTest()
    .then(() => console.log('ok'))
    .catch((err) => console.log(err));
