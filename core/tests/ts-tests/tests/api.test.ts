import { Wallet, RestProvider, getDefaultRestProvider, types, utils } from '@rsksmart/rif-aggregation-sdk-js';
import { Tester } from './tester';
import * as ethers from 'ethers';
import './priority-ops';
import './change-pub-key';
import './transfer';
import './withdraw';
import './forced-exit';
import './mint-nft';
import { expect } from 'chai';
import path from 'path';

import * as api from './api';

describe('ZkSync REST API V0.1 tests', () => {
    let tester: Tester;
    let alice: Wallet;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', 'HTTP', 'RPC');
        alice = await tester.fundedWallet('1.0');
        let bob = await tester.emptyWallet();
        for (const token of ['ETH', 'DAI']) {
            const thousand = tester.syncProvider.tokenSet.parseToken(token, '1000');
            await tester.testDeposit(alice, token, thousand, true);
            await tester.testChangePubKey(alice, token, false);
            await tester.testTransfer(alice, bob, token, thousand.div(4));
            await tester.testForcedExit(alice, bob, token);
            await tester.testWithdraw(alice, token, thousand.div(5));
            await tester.testFullExit(alice, token);
            await tester.testDeposit(alice, token, thousand.div(10), true);
        }
        api.deleteUnusedGenFiles();
    });

    after('disconnect tester', async () => {
        api.deleteUnusedGenFiles();
        await tester.disconnect();
    });

    it('should check status response type', async () => {
        await api.checkStatusResponseType();
    });

    it('should check testnet config response type', async () => {
        await api.checkTestnetConfigResponseType();
    });

    it('should check withdrawal processing time response type', async () => {
        await api.checkWithdrawalProcessingTimeResponseType();
    });

    it('should check tx history response type', async () => {
        await api.checkTxHistoryResponseType(alice.address());
    });

    it('should check blocks response type', async () => {
        const blocksToCheck = 10;
        const blocks = await api.checkBlocksResponseType();
        for (const { block_number } of blocks.slice(-blocksToCheck)) {
            await api.checkBlockResponseType(block_number);
            const txs = await api.checkBlockTransactionsResponseType(block_number);
            for (const { tx_hash } of txs) {
                await api.checkTransactionsResponseType(tx_hash);
            }
        }
    });
});

describe('ZkSync REST API V0.2 tests', () => {
    let tester: Tester;
    let alice: Wallet;
    let bob: Wallet;
    let provider: RestProvider;
    let lastTxHash: string;
    let lastTxReceipt: types.TransactionReceipt;

    before('create tester and test wallets', async () => {
        provider = await getDefaultRestProvider('localhost');
        tester = await Tester.init('localhost', 'HTTP', 'REST');
        alice = await tester.fundedWallet('1.0');
        bob = await tester.emptyWallet();
        for (const token of ['ETH', 'wBTC']) {
            const thousand = tester.syncProvider.tokenSet.parseToken(token, '1000');
            await tester.testDeposit(alice, token, thousand, true);
            if (token === 'ETH') await tester.testChangePubKey(alice, token, false);
            await tester.testTransfer(alice, bob, token, thousand.div(4));
        }

        const handle = await alice.syncTransfer({
            to: bob.address(),
            token: 'ETH',
            amount: alice.provider.tokenSet.parseToken('ETH', '1')
        });
        lastTxHash = handle.txHash;
        lastTxHash.replace('sync-tx:', '0x');
        lastTxReceipt = await handle.awaitReceipt();
    });

    it('should check api v0.2 account scope', async () => {
        const committedState = await provider.accountInfo(alice.address(), 'committed');
        const finalizedState = await provider.accountInfo(alice.address(), 'finalized');
        const fullState = await provider.accountFullInfo(alice.address());
        expect(fullState.committed, 'committed state differs').to.eql(committedState);
        expect(fullState.finalized, 'finalized state differs').to.eql(finalizedState);

        const expectedETHTxs = 4;
        const expectedWBTCTxs = 2;
        const expectedAll = expectedETHTxs + expectedWBTCTxs;

        const ethTxs = await provider.accountTxs(
            alice.accountId!,
            {
                from: lastTxHash,
                limit: 10,
                direction: 'older'
            },
            'ETH'
        );
        expect(
            ethTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${ethTxs.list.length}, expected ${expectedETHTxs}`
        ).to.eql(expectedETHTxs);

        const wbtcTxs = await provider.accountTxs(
            alice.accountId!,
            {
                from: lastTxHash,
                limit: 10,
                direction: 'older'
            },
            'wBTC'
        );
        expect(
            wbtcTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${wbtcTxs.list.length}, expected ${expectedETHTxs}`
        ).to.eql(expectedWBTCTxs);

        const allTxs = await provider.accountTxs(alice.accountId!, {
            from: lastTxHash,
            limit: 10,
            direction: 'older'
        });
        expect(
            allTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${allTxs.list.length}, expected ${expectedAll}`
        ).to.eql(expectedAll);
        expect(allTxs.list[0].txHash, 'Endpoint did not return first tx correctly').to.be.eql(lastTxHash);

        const expectedAliceBob = 3;
        const expectedAliceTester = 2;

        const aliceAndBobTxs = await provider.accountTxs(
            alice.accountId!,
            {
                from: lastTxHash,
                limit: 10,
                direction: 'older'
            },
            undefined,
            bob.address()
        );
        expect(
            aliceAndBobTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${aliceAndBobTxs.list.length}, expected ${expectedAliceBob}`
        ).to.eql(expectedAliceBob);

        const aliceAndTesterTxs = await provider.accountTxs(
            alice.accountId!,
            {
                from: lastTxHash,
                limit: 10,
                direction: 'older'
            },
            undefined,
            tester.syncWallet.address()
        );
        expect(
            aliceAndTesterTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${aliceAndTesterTxs.list.length}, expected ${expectedAliceTester}`
        ).to.eql(expectedAliceTester);

        const accTxs = await provider.accountPendingTxs(alice.accountId!, {
            from: 1,
            limit: 10,
            direction: 'newer'
        });
        expect(accTxs).to.exist;
    });

    it('should check api v0.2 block scope', async () => {
        const lastCommittedBlock = await provider.blockByPosition('lastCommitted');
        expect(lastCommittedBlock).to.exist;

        const expectedBlocks = 3;
        const blocks = await provider.blockPagination({
            from: lastCommittedBlock.blockNumber,
            limit: 3,
            direction: 'older'
        });
        expect(
            blocks.list.length,
            `Endpoint returned incorrect number of blocks: ${blocks.list.length}, expected ${expectedBlocks}`
        ).to.eql(expectedBlocks);

        const expectedTxs = 1;
        const blockTxs = await provider.blockTransactions(lastTxReceipt.block!.blockNumber, {
            from: lastTxHash,
            limit: 10,
            direction: 'newer'
        });
        expect(
            blockTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${blockTxs.list.length}, expected ${expectedTxs}`
        ).to.eql(expectedTxs);
    });

    it('should check api v0.2 config endpoint', async () => {
        const config = await provider.config();
        expect(config.network === 'localhost').to.be.true;
    });

    it('should check api v0.2 fee scope', async () => {
        const fee = await provider.getTransactionFee('Withdraw', alice.address(), 'ETH');
        expect(fee).to.exist;
        const batchFee = await provider.getBatchFullFee(
            [
                { txType: 'Transfer', address: alice.address() },
                { txType: 'Withdraw', address: alice.address() }
            ],
            'ETH'
        );
        expect(batchFee).to.exist;
    });

    it('should check api v0.2 network status endpoint', async () => {
        const networkStatus = await provider.networkStatus();
        expect(networkStatus).to.exist;
    });

    it('should check api v0.2 token scope', async () => {
        const tokens = await provider.tokenPagination({
            from: 0,
            limit: 2,
            direction: 'newer'
        });
        expect(tokens.list.length).to.be.eql(2);
        const firstToken = await provider.tokenInfo('0x'.padEnd(42, '0'));
        const secondToken = await provider.tokenInfo(1);
        expect(tokens.list[0]).to.be.eql(firstToken);
        expect(tokens.list[1]).to.be.eql(secondToken);
        const firstTokenUSDPrice = await provider.getTokenPrice(0);
        const secondTokenUSDPrice = await provider.getTokenPrice(1);
        const expectedPrice = firstTokenUSDPrice / secondTokenUSDPrice;
        const actualPrice = parseFloat((await provider.tokenPriceInfo(0, 1)).price);
        expect(expectedPrice).to.be.lessThan(1.05 * actualPrice);
        expect(expectedPrice).to.be.greaterThan(0.95 * actualPrice);
    });

    it('should check api v0.2 transaction scope', async () => {
        const apiReceipt = await provider.txStatus(lastTxHash);
        expect(apiReceipt!.rollupBlock).to.exist;

        const txData = await provider.txData(lastTxHash);
        expect(txData!.tx.op.type).to.eql('Transfer');
        expect(txData!.tx.batchId).to.not.exist;

        const batch = await alice
            .batchBuilder()
            .addTransfer({ to: bob.address(), token: 'ETH', amount: alice.provider.tokenSet.parseToken('ETH', '1') })
            .addTransfer({ to: bob.address(), token: 'ETH', amount: alice.provider.tokenSet.parseToken('ETH', '1') })
            .build('ETH');
        const submitBatchResponse = await provider.submitTxsBatchNew(batch.txs, [batch.signature]);
        await provider.notifyAnyTransaction(submitBatchResponse.transactionHashes[0], 'COMMIT');
        const batchInfo = await provider.getBatch(submitBatchResponse.batchHash);
        expect(batchInfo.batchHash).to.eql(submitBatchResponse.batchHash);

        const txInBatchData1 = await provider.txData(batchInfo.transactionHashes[0]);
        const txInBatchData2 = await provider.txData(batchInfo.transactionHashes[1]);
        expect(txInBatchData1.tx.batchId).to.exist;
        expect(txInBatchData1.tx.batchId).to.eql(txInBatchData2.tx.batchId);
    });
});

describe('ZkSync web3 API tests', () => {
    let tester: Tester;
    let alice: Wallet;
    let bob: Wallet;
    const token: string = 'ETH';
    let depositAmount: ethers.BigNumber;
    let web3Provider: ethers.ethers.providers.BaseProvider;
    let restProvider: RestProvider;
    let tokenAddress: string;
    let erc20Contract: ethers.Contract;
    const zksyncProxyAddress = '0x1000000000000000000000000000000000000000';
    let zksyncProxyContract: ethers.Contract;
    const nftFactoryAddress = '0x2000000000000000000000000000000000000000';
    let nftFactoryContract: ethers.Contract;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', 'HTTP', 'RPC');
        alice = await tester.fundedWallet('1.0');
        bob = await tester.emptyWallet();
        restProvider = await getDefaultRestProvider('localhost');
        web3Provider = new ethers.providers.JsonRpcProvider('http://localhost:3002');
        depositAmount = tester.syncProvider.tokenSet.parseToken(token, '1000');
        await tester.testDeposit(alice, token, depositAmount, true);
        await tester.testChangePubKey(alice, token, false);

        tokenAddress = alice.provider.tokenSet.resolveTokenAddress(token);
        const erc20InterfacePath = path.join(process.env['ZKSYNC_HOME'] as string, 'etc', 'web3-abi', 'ERC20.json');
        const erc20Interface = new ethers.utils.Interface(require(erc20InterfacePath));
        erc20Contract = new ethers.Contract(tokenAddress, erc20Interface, alice.ethSigner);

        const zksyncProxyInterfacePath = path.join(
            process.env['ZKSYNC_HOME'] as string,
            'etc',
            'web3-abi',
            'ZkSyncProxy.json'
        );
        const zksyncProxyInterface = new ethers.utils.Interface(require(zksyncProxyInterfacePath));
        zksyncProxyContract = new ethers.Contract(zksyncProxyAddress, zksyncProxyInterface, alice.ethSigner);

        const nftFactoryInterfacePath = path.join(
            process.env['ZKSYNC_HOME'] as string,
            'etc',
            'web3-abi',
            'NFTFactory.json'
        );
        const nftFactoryInterface = new ethers.utils.Interface(require(nftFactoryInterfacePath));
        nftFactoryContract = new ethers.Contract(nftFactoryAddress, nftFactoryInterface, alice.ethSigner);
    });

    it('should check logs', async () => {
        const fee = (await restProvider.getTransactionFee('Transfer', bob.address(), 'ETH')).totalFee;
        const transferAmount = depositAmount.div(10);
        const handle = await alice.syncTransfer({
            to: bob.address(),
            token,
            amount: transferAmount,
            fee
        });
        const txHash = handle.txHash.replace('sync-tx:', '0x');
        const receipt = await handle.awaitReceipt();
        const blockNumber = receipt.block!.blockNumber;

        // Wait until the block is committed confirmed.
        let committedConfirmed: number;
        do {
            await utils.sleep(1000);
            committedConfirmed = (await restProvider.networkStatus()).lastCommitted;
        } while (committedConfirmed < blockNumber);

        const web3Receipt = await web3Provider.getTransactionReceipt(txHash);
        expect(web3Receipt.logs.length, 'Incorrect number of logs').to.eql(3);

        const zksyncTransferSignature = 'ZkSyncTransfer(address,address,address,uint256,uint256)';
        const zksyncTransferTopic = ethers.utils.keccak256(ethers.utils.toUtf8Bytes(zksyncTransferSignature));
        const erc20TransferSignature = 'Transfer(address,address,uint256)';
        const erc20TransferTopic = ethers.utils.keccak256(ethers.utils.toUtf8Bytes(erc20TransferSignature));

        const expectedTopicsAndEvents = [
            [zksyncTransferTopic, zksyncProxyContract, zksyncTransferSignature],
            [erc20TransferTopic, erc20Contract, erc20TransferSignature],
            [erc20TransferTopic, erc20Contract, erc20TransferSignature]
        ];
        const expectedData = [
            {
                from: alice.address(),
                to: bob.address(),
                token: tokenAddress,
                amount: transferAmount,
                fee
            },
            {
                from: alice.address(),
                to: bob.address(),
                value: transferAmount
            },
            {
                from: alice.address(),
                to: '0x'.padEnd(42, '0'),
                value: fee
            }
        ];
        for (let i = 0; i < 3; ++i) {
            expect(web3Receipt.logs[i].topics.length, 'Incorrect number of topics').to.eql(1);
            expect(web3Receipt.logs[i].topics[0], 'Incorrect topic').to.eql(expectedTopicsAndEvents[i][0]);
            const contract = expectedTopicsAndEvents[i][1] as ethers.Contract;
            const eventSignature = expectedTopicsAndEvents[i][2] as string;
            const log = contract.interface.decodeEventLog(eventSignature, web3Receipt.logs[i].data);
            for (const key in expectedData[i]) {
                const expected = (expectedData[i] as any)[key];
                expect(log[key], 'Incorrect data').to.eql(expected);
            }
        }
    }).timeout(1000000);

    it('should check erc721 calls', async () => {
        const contentHash = '0x218145f24cb870cc72ec7f0cc734b86f3e9a744666282f99023f022be77aaea6';
        const mintNFTHandle = await alice.mintNFT({
            recipient: alice.address(),
            contentHash,
            feeToken: token
        });
        await mintNFTHandle.awaitVerifyReceipt();
        const state = await alice.getAccountState();
        const nft = Object.values(state.committed.nfts)[0];

        const ownerOfFunction = nftFactoryContract.interface.functions['ownerOf(uint256)'];
        const ownerOfCallData = nftFactoryContract.interface.encodeFunctionData(ownerOfFunction, [nft.id]);

        let callResult = await web3Provider.call({ to: nftFactoryAddress, data: ownerOfCallData });
        const owner1 = nftFactoryContract.interface.decodeFunctionResult(ownerOfFunction, callResult)[0];
        expect(owner1, 'Incorrect owner after mint').to.eql(alice.address());

        const transferHandle = (await alice.syncTransferNFT({ to: bob.address(), token: nft, feeToken: 'ETH' }))[0];
        await transferHandle.awaitVerifyReceipt();

        callResult = await web3Provider.call({ to: nftFactoryAddress, data: ownerOfCallData });
        const owner2 = nftFactoryContract.interface.decodeFunctionResult(ownerOfFunction, callResult)[0];
        expect(owner2, 'Incorrect owner after transfer').to.eql(bob.address());

        const tokenURIFunction = nftFactoryContract.interface.functions['tokenURI(uint256)'];
        const tokenURICallData = nftFactoryContract.interface.encodeFunctionData(tokenURIFunction, [nft.id]);
        callResult = await web3Provider.call({ to: nftFactoryAddress, data: tokenURICallData });
        const tokenURI = nftFactoryContract.interface.decodeFunctionResult(tokenURIFunction, callResult)[0];
        const expectedURI = 'ipfs://QmQbSVaG7DUjQ9ktPtMnSXReJ29XHezBghcxJeZDsGG7wB';
        expect(tokenURI, 'Incorrect token URI').to.eql(expectedURI);
    });

    it('should check erc20 calls', async () => {
        const balanceOfFunction = erc20Contract.interface.functions['balanceOf(address)'];
        const balanceOfCallData = erc20Contract.interface.encodeFunctionData(balanceOfFunction, [alice.address()]);
        let callResult = await web3Provider.call({ to: tokenAddress, data: balanceOfCallData });
        let balance = nftFactoryContract.interface.decodeFunctionResult(balanceOfFunction, callResult)[0];
        let expectedBalance = (await alice.getAccountState()).verified.balances[token] as string;
        expect(balance.toString(), 'Incorrect balance before transfer').to.eql(expectedBalance);

        const transferAmount = depositAmount.div(10);
        const handle = await alice.syncTransfer({
            to: bob.address(),
            token,
            amount: transferAmount
        });
        await handle.awaitVerifyReceipt();

        callResult = await web3Provider.call({ to: tokenAddress, data: balanceOfCallData });
        balance = nftFactoryContract.interface.decodeFunctionResult(balanceOfFunction, callResult)[0];
        expectedBalance = (await alice.getAccountState()).verified.balances[token] as string;
        expect(balance.toString(), 'Incorrect balance after transfer').to.eql(expectedBalance);
    });

    it('should check eth_call error', async () => {
        const ownerOfFunction = nftFactoryContract.interface.functions['ownerOf(uint256)'];
        const ownerOfCallData = nftFactoryContract.interface.encodeFunctionData(ownerOfFunction, [99999]);
        let failed = false;
        try {
            await web3Provider.call({ to: nftFactoryAddress, data: ownerOfCallData });
        } catch (e) {
            expect(e.toString().includes('execution reverted: ERC721: owner query for nonexistent token')).to.be.true;
            failed = true;
        }
        expect(failed).to.be.true;
    });
});
