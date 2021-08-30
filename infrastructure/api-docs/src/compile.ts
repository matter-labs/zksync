import * as fs from 'fs';
import * as path from 'path';
import * as handlebars from 'handlebars';
import * as zksync from 'zksync';
import * as ethers from 'ethers';

export function getDirPath() {
    return path.join(process.env.ZKSYNC_HOME as string, 'infrastructure/api-docs');
}

function pasteAllFilesInOne() {
    let template = handlebars.compile(fs.readFileSync(path.join(getDirPath(), 'blueprint/template.apib'), 'utf-8'), {
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

async function compileCommon() {
    const data = pasteAllFilesInOne();
    let template = handlebars.compile(data, { noEscape: true });

    let replaceObject: any = await getHashesAndSignatures();
    replaceObject['isResultNullable'] = '{{isResultNullable}}';

    return template(replaceObject);
}

async function setupWallet() {
    const pathToConfig = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant/eth.json`);
    const config = fs.readFileSync(pathToConfig, {
        encoding: 'utf-8'
    });
    const ethTestConfig = JSON.parse(config);
    let web3Url = (process.env.ETH_CLIENT_WEB3_URL as string).split(',')[0];
    const ethProvider = new ethers.providers.JsonRpcProvider(web3Url);
    ethProvider.pollingInterval = 100;
    const syncProvider = await zksync.getDefaultRestProvider('localhost');
    const ethWallet = new ethers.Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow_privK, 'hex'), ethProvider);

    const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);

    const depositHandle = await syncWallet.depositToSyncFromEthereum({
        depositTo: syncWallet.address(),
        token: 'ETH',
        amount: syncWallet.provider.tokenSet.parseToken('ETH', '1000')
    });
    await depositHandle.awaitReceipt();

    if (!(await syncWallet.isSigningKeySet())) {
        const changePubkeyHandle = await syncWallet.setSigningKey({
            feeToken: 'ETH',
            ethAuthType: 'ECDSA'
        });
        await changePubkeyHandle.awaitReceipt();
    }

    return syncWallet;
}

interface Parameters {
    txHash: string;
    txBatchHash: string;
    address: string;
    accountId: number;
    pubKey: string;
    l2Signature: string;
    ethereumSignature: string;
    nftId: number;
    toggle2FASignature: string;
    toggle2FATimestamp: number;
}

async function getHashesAndSignatures() {
    let syncWallet = await setupWallet();

    const handle = await syncWallet.syncTransfer({ to: syncWallet.address(), token: 'ETH', amount: 0 });
    await handle.awaitReceipt();
    const txHash = handle.txHash;

    const batch = await syncWallet
        .batchBuilder()
        .addTransfer({ to: syncWallet.address(), token: 'ETH', amount: 0 })
        .build('ETH');

    const submitBatchResponse = await (syncWallet.provider as zksync.RestProvider).submitTxsBatchNew(
        batch.txs,
        batch.signature
    );
    await syncWallet.provider.notifyTransaction(submitBatchResponse.transactionHashes[0], 'COMMIT');
    const txBatchHash = submitBatchResponse.batchHash;

    const signedTransfer = await syncWallet.signSyncTransfer({
        to: '0xD3c62D2F7b6d4A63577F2415E55A6Aa6E1DbB9CA',
        token: 'ETH',
        amount: '17500000000000000',
        fee: '12000000000000000000',
        nonce: 12123,
        validFrom: 0,
        validUntil: 1239213821
    });
    const address = syncWallet.address();
    const accountId = (await syncWallet.getAccountId())!;
    const pubKey = signedTransfer.tx.signature!.pubKey;
    const l2Signature = signedTransfer.tx.signature!.signature;
    const ethereumSignature = (signedTransfer.ethereumSignature as zksync.types.TxEthSignature).signature;

    const mintHandle = await syncWallet.mintNFT({
        recipient: address,
        contentHash: ethers.utils.randomBytes(32),
        feeToken: 'ETH'
    });
    await mintHandle.awaitVerifyReceipt();
    const state = await syncWallet.getAccountState();
    const nftId = Object.values(state.verified.nfts)[0].id;

    const toggle2FAObject = await syncWallet.getToggle2FA(false);
    const toggle2FASignature = (toggle2FAObject.signature as zksync.types.TxEthSignature).signature;
    const toggle2FATimestamp = toggle2FAObject.timestamp;

    let result: Parameters = {
        txHash,
        txBatchHash,
        address,
        accountId,
        pubKey,
        l2Signature,
        ethereumSignature,
        nftId,
        toggle2FASignature,
        toggle2FATimestamp
    };
    return result;
}

export async function compileApibForDocumentation() {
    const before = await compileCommon();
    let template = handlebars.compile(before, { noEscape: true });

    let replaceObject: any = {};
    replaceObject['isResultNullable'] = ', nullable';

    const after = template(replaceObject);

    fs.writeFileSync(path.join(getDirPath(), 'blueprint/documentation.apib'), after);
}

export async function compileApibForTest() {
    const before = await compileCommon();
    let template = handlebars.compile(before, { noEscape: true });

    let replaceObject: any = {};
    replaceObject['isResultNullable'] = '';

    const after = template(replaceObject);

    fs.writeFileSync(path.join(getDirPath(), 'blueprint/test.apib'), after);
}
