import { Contract, ethers, constants, BigNumber } from 'ethers';
import { keccak256, parseEther } from 'ethers/lib/utils';
import { ETHProxy } from 'zksync';
import { Address, TokenAddress } from 'zksync/build/types';
import { Deployer, readContractCode, readProductionContracts } from '../../src.ts/deploy';

const hardhat = require('hardhat');
const { simpleEncode } = require('ethereumjs-abi');
const { expect } = require('chai');
const { getCallRevertReason, IERC20_INTERFACE, DEFAULT_REVERT_REASON } = require('./common');
import * as zksync from 'zksync';
import {
    ZkSync,
    ZkSyncProcessOpUnitTest,
    ZkSyncProcessOpUnitTestFactory,
    ZKSyncSignatureUnitTest,
    ZKSyncSignatureUnitTestFactory,
    ZkSyncWithdrawalUnitTestFactory
} from '../../typechain';

const TEST_PRIORITY_EXPIRATION = 101;
const CHUNK_SIZE = 10;

let wallet;

describe('zkSync signature verification unit tests', function () {
    this.timeout(50000);

    let testContract: ZKSyncSignatureUnitTest;
    const randomWallet = ethers.Wallet.createRandom();
    before(async () => {
        [wallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZKSyncSignatureUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });

        testContract = ZKSyncSignatureUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);
    });

    it('pubkey hash signature verification success', async () => {
        const pubkeyHash = 'sync:fefefefefefefefefefefefefefefefefefefefe';
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const witness = ethers.utils.concat(['0x00', signature]);
        const { result } = await getCallRevertReason(
            async () =>
                await testContract.changePubkeySignatureCheckECRECOVER(
                    { accountId, owner: randomWallet.address, nonce, pubKeyHash: pubkeyHash.replace('sync:', '0x') },
                    witness
                )
        );
        expect(result).eq(true);
    });

    it('pubkey hash signature verification incorrect nonce', async () => {
        const incorrectNonce = 0x11223345;
        const pubkeyHash = 'sync:fefefefefefefefefefefefefefefefefefefefe';
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const witness = ethers.utils.concat(['0x00', signature]);
        const { result } = await getCallRevertReason(
            async () =>
                await testContract.changePubkeySignatureCheckECRECOVER(
                    {
                        accountId,
                        owner: randomWallet.address,
                        nonce: incorrectNonce,
                        pubKeyHash: pubkeyHash.replace('sync:', '0x')
                    },
                    witness
                )
        );
        expect(result).eq(false);
    });

    it('pubkey hash signature verification incorrect pubkey hash', async () => {
        const incorrectPubkeyHash = 'sync:aaaafefefefefefefefefefefefefefefefefefe';
        const pubkeyHash = 'sync:fefefefefefefefefefefefefefefefefefefefe';
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const witness = ethers.utils.concat(['0x00', signature]);
        const { result } = await getCallRevertReason(
            async () =>
                await testContract.changePubkeySignatureCheckECRECOVER(
                    {
                        accountId,
                        owner: randomWallet.address,
                        nonce,
                        pubKeyHash: incorrectPubkeyHash.replace('sync:', '0x')
                    },
                    witness
                )
        );
        expect(result).eq(false);
    });

    it('pubkey hash signature verification incorrect signer', async () => {
        const incorrectSignerAddress = wallet.address;
        const pubkeyHash = 'sync:fefefefefefefefefefefefefefefefefefefefe';
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const witness = ethers.utils.concat(['0x00', signature]);
        const { result } = await getCallRevertReason(
            async () =>
                await testContract.changePubkeySignatureCheckECRECOVER(
                    { accountId, owner: incorrectSignerAddress, nonce, pubKeyHash: pubkeyHash.replace('sync:', '0x') },
                    witness
                )
        );
        expect(result).eq(false);
    });

    it('pubkey hash signature verification incorrect account id', async () => {
        const incorrectAccountId = 0xbabeba;
        const pubkeyHash = 'sync:fefefefefefefefefefefefefefefefefefefefe';
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const witness = ethers.utils.concat(['0x00', signature]);
        const { result } = await getCallRevertReason(
            async () =>
                await testContract.changePubkeySignatureCheckECRECOVER(
                    {
                        accountId: incorrectAccountId,
                        owner: wallet.address,
                        nonce,
                        pubKeyHash: pubkeyHash.replace('sync:', '0x')
                    },
                    witness
                )
        );
        expect(result).eq(false);
    });

    it('signature verification success', async () => {
        for (const message of [Buffer.from('msg', 'ascii'), Buffer.alloc(0), Buffer.alloc(10, 1)]) {
            const signature = await wallet.signMessage(message);
            const signedMessageHash = ethers.utils.keccak256(
                Buffer.concat([Buffer.from(`\x19Ethereum Signed Message:\n${message.length}`, 'ascii'), message])
            );
            const address = await testContract.testRecoverAddressFromEthSignature(signature, signedMessageHash);
            expect(address, `address mismatch, message ${message.toString('hex')}`).eq(wallet.address);
        }
    });
});

describe('ZK priority queue ops unit tests', function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    before(async () => {
        [wallet] = await hardhat.ethers.getSigners();
        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncProcessOpUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncProcessOpUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractFactory = await hardhat.ethers.getContractFactory('TestnetERC20Token');
        tokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);
    });

    async function performDeposit(to: Address, token: TokenAddress, depositAmount: BigNumber) {
        const openedRequests = await zksyncContract.getTotalOpenPriorityRequests();
        const depositOwner = wallet.address;

        let tx;
        if (token === ethers.constants.AddressZero) {
            tx = await zksyncContract.depositETH(depositOwner, {
                value: depositAmount
            });
        } else {
            tx = await zksyncContract.depositERC20(token, depositAmount, depositOwner);
        }
        const receipt = await tx.wait();

        const deadlineBlock = receipt.blockNumber + TEST_PRIORITY_EXPIRATION;

        let priorityQueueEvent;
        for (const event of receipt.logs) {
            try {
                const parsedLog = zksyncContract.interface.parseLog(event);
                if (parsedLog && parsedLog.name === 'NewPriorityRequest') {
                    priorityQueueEvent = parsedLog;
                    break;
                }
            } catch {}
        }
        expect(priorityQueueEvent.name, 'event name').eq('NewPriorityRequest');
        expect(priorityQueueEvent.args.sender, 'sender address').eq(wallet.address);
        expect(priorityQueueEvent.args.serialId, 'request id').eq(openedRequests);
        expect(priorityQueueEvent.args.opType, 'request type').eq(1);
        expect(priorityQueueEvent.args.expirationBlock, 'expiration block').eq(deadlineBlock);
    }

    async function performFullExitRequest(accountId: number, token: TokenAddress) {
        const openedRequests = await zksyncContract.getTotalOpenPriorityRequests();
        const tx = await zksyncContract.requestFullExit(accountId, token);
        const receipt = await tx.wait();

        const deadlineBlock = receipt.blockNumber + TEST_PRIORITY_EXPIRATION;

        let priorityQueueEvent;
        for (const event of receipt.logs) {
            try {
                const parsedLog = zksyncContract.interface.parseLog(event);
                if (parsedLog && parsedLog.name === 'NewPriorityRequest') {
                    priorityQueueEvent = parsedLog;
                    break;
                }
            } catch {}
        }
        expect(priorityQueueEvent.name, 'event name').eq('NewPriorityRequest');
        expect(priorityQueueEvent.args.sender, 'sender address').eq(wallet.address);
        expect(priorityQueueEvent.args.serialId, 'request id').eq(openedRequests);
        expect(priorityQueueEvent.args.opType, 'request type').eq(6);
        expect(priorityQueueEvent.args.expirationBlock, 'expiration block').eq(deadlineBlock);
    }

    it('success ETH deposits', async () => {
        zksyncContract.connect(wallet);
        const tokenAddress = ethers.constants.AddressZero;
        const depositAmount = parseEther('1.0');

        await performDeposit(wallet.address, tokenAddress, depositAmount);
        await performDeposit(ethers.Wallet.createRandom().address, tokenAddress, depositAmount);
    });

    it('success ERC20 deposits', async () => {
        zksyncContract.connect(wallet);
        const tokenAddress = tokenContract.address;
        const depositAmount = parseEther('1.0');

        tokenContract.connect(wallet);
        await tokenContract.approve(zksyncContract.address, depositAmount);
        await performDeposit(wallet.address, tokenAddress, depositAmount);
        await tokenContract.approve(zksyncContract.address, depositAmount);
        await performDeposit(ethers.Wallet.createRandom().address, tokenAddress, depositAmount);
    });

    it('success FullExit request', async () => {
        zksyncContract.connect(wallet);
        const accountId = 1;

        await performFullExitRequest(accountId, ethers.constants.AddressZero);
        await performFullExitRequest(accountId, tokenContract.address);
    });
});

async function onchainBalance(ethWallet: ethers.Wallet, token: Address): Promise<BigNumber> {
    if (token === ethers.constants.AddressZero) {
        return ethWallet.getBalance();
    } else {
        const erc20contract = new Contract(token, IERC20_INTERFACE.abi, ethWallet);
        return BigNumber.from(await erc20contract.balanceOf(ethWallet.address));
    }
}

describe('zkSync withdraw unit tests', function () {
    this.timeout(50000);

    let zksyncContract: ZkSync;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;
    before(async () => {
        [wallet] = await hardhat.ethers.getSigners();
        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncWithdrawalUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });

        zksyncContract = ZkSyncWithdrawalUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractFactory = await hardhat.ethers.getContractFactory('TestnetERC20Token');
        tokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address
        });

        incorrectTokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await incorrectTokenContract.mint(wallet.address, parseEther('1000000'));
    });

    async function performWithdraw(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        const contractBalanceBefore = BigNumber.from(await zksyncContract.getPendingBalance(ethWallet.address, token));
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawPendingBalance(ethWallet.address, token, amount, {
                gasLimit: 300000
            });
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawPendingBalance(ethWallet.address, token, amount, { gasLimit: 300000 });
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance =
            token == constants.AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), 'withdraw account balance mismatch').eq(expectedBalance.toString());

        const contractBalanceAfter = BigNumber.from(await zksyncContract.getPendingBalance(ethWallet.address, token));
        const expectedContractBalance = contractBalanceBefore.sub(amount);
        expect(contractBalanceAfter.toString(), 'withdraw contract balance mismatch').eq(
            expectedContractBalance.toString()
        );
    }

    it('Withdraw ETH success', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const sendETH = await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount.mul(2),
            data: simpleEncode('receiveETH()')
        });
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount);

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount.div(2));
        await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount.div(2));
    });

    it('Withdraw ETH incorrect amount', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const sendETH = await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount,
            data: simpleEncode('receiveETH()')
        });
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        const { revertReason } = await getCallRevertReason(
            async () => await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount.add(1))
        );
        expect(revertReason, 'wrong revert reason').eq('aa');
    });

    it('Withdraw ERC20 success', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount.mul(2));
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.div(2));
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.div(2));
    });

    it('Withdraw ERC20 incorrect amount', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount);
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);

        const onchainBalBefore = await onchainBalance(wallet, tokenContract.address);
        try {
            const { revertReason } = await getCallRevertReason(
                async () => await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.add(1))
            );
            expect(revertReason).to.not.eq(DEFAULT_REVERT_REASON);
        } catch (err) {}
        const onchainBalAfter = await onchainBalance(wallet, tokenContract.address);

        expect(onchainBalAfter).eq(onchainBalBefore);
    });

    it('Withdraw ERC20 unsupported token', async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther('1.0');

        const { revertReason } = await getCallRevertReason(
            async () => await performWithdraw(wallet, incorrectTokenContract.address, 1, withdrawAmount.add(1))
        );
        expect(revertReason, 'wrong revert reason').eq('1i');
    });
});

describe('zkSync auth pubkey onchain unit tests', function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    before(async () => {
        [wallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncProcessOpUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncProcessOpUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractFactory = await hardhat.ethers.getContractFactory('TestnetERC20Token');
        tokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);
    });

    it('Auth pubkey success', async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = '0xfefefefefefefefefefefefefefefefefefefefe';

        await (await zksyncContract.setAuthPubkeyHash(pubkeyHash, nonce)).wait();

        const expectedAuthFact = ethers.utils.keccak256(pubkeyHash);

        const authFact = await zksyncContract.getAuthFact(wallet.address, nonce);
        expect(authFact).to.eq(expectedAuthFact);
    });

    it('Auth pubkey reset', async () => {
        zksyncContract.connect(wallet);

        const checkSetPubkeyHash = async (pubkeyHash, address, nonce, message) => {
            const expectedAuthFact = ethers.utils.keccak256(pubkeyHash);
            const authFact = await zksyncContract.getAuthFact(address, nonce);
            expect(authFact).to.eq(expectedAuthFact, message);
        };

        const nonce = 0x5678;
        const pubkeyHash = '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

        await (await zksyncContract.setAuthPubkeyHash(pubkeyHash, nonce)).wait();
        await checkSetPubkeyHash(pubkeyHash, wallet.address, nonce, 'first pubkey hash set');

        const resetPubkeyHash = '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

        await (await zksyncContract.setAuthPubkeyHash(resetPubkeyHash, nonce)).wait();
        await checkSetPubkeyHash(pubkeyHash, wallet.address, nonce, 'first pubkey hash still set');
        const resetTimestamp = Date.now() + 24 * 60 * 60;

        await zksyncContract.provider.send('evm_setNextBlockTimestamp', [resetTimestamp]);
        await (await zksyncContract.setAuthPubkeyHash(resetPubkeyHash, nonce)).wait();
        await checkSetPubkeyHash(resetPubkeyHash, wallet.address, nonce, 'pubkey hash changed');
    });

    it('Auth pubkey incorrect length fail', async () => {
        zksyncContract.connect(wallet);
        const nonce = 0x7656;
        const shortPubkeyHash = '0xfefefefefefefefefefefefefefefefefefefe';
        const longPubkeyHash = '0xfefefefefefefefefefefefefefefefefefefefefe';

        for (const pkHash of [shortPubkeyHash, longPubkeyHash]) {
            const { revertReason } = await getCallRevertReason(
                async () =>
                    await zksyncContract.setAuthPubkeyHash(pkHash, nonce, {
                        gasLimit: 300000
                    })
            );
            expect(revertReason, 'revert reason incorrect').eq('y');
        }
    });
});

describe('zkSync test process next operation', function () {
    this.timeout(50000);

    let zksyncContract: ZkSyncProcessOpUnitTest;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;

    const EMPTY_KECCAK = ethers.utils.keccak256('0x');

    const newBlockDataFromPubdata = (publicData) => {
        return {
            blockNumber: 0,
            feeAccount: 0,
            newStateHash: ethers.constants.HashZero,
            publicData,
            timestamp: 0,
            onchainOperations: []
        };
    };

    before(async () => {
        [wallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncProcessOpUnitTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = ZkSyncProcessOpUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractFactory = await hardhat.ethers.getContractFactory('TestnetERC20Token');
        tokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address
        });

        incorrectTokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await incorrectTokenContract.mint(wallet.address, parseEther('1000000'));
    });

    it('Process noop', async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.getTotalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(CHUNK_SIZE, 0);
        const blockData = newBlockDataFromPubdata(pubdata);

        await zksyncContract.collectOnchainOpsExternal(blockData, EMPTY_KECCAK, 0, [0]);

        const committedPriorityRequestsAfter = await zksyncContract.getTotalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, 'priority request number').eq(committedPriorityRequestsBefore);
    });

    it('Process transfer', async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.getTotalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(CHUNK_SIZE * 2, 0xff);
        pubdata[0] = 0x05;
        const blockData = newBlockDataFromPubdata(pubdata);
        await zksyncContract.collectOnchainOpsExternal(blockData, EMPTY_KECCAK, 0, [0, 0]);

        const committedPriorityRequestsAfter = await zksyncContract.getTotalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, 'priority request number').eq(committedPriorityRequestsBefore);
    });
    it('Process transfer to new', async () => {
        zksyncContract.connect(wallet);

        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0xff);
        pubdata[0] = 0x02;
        const blockData = newBlockDataFromPubdata(pubdata);
        await zksyncContract.collectOnchainOpsExternal(blockData, EMPTY_KECCAK, 0, [0, 0, 0, 0, 0, 0]);
    });

    it('Process deposit', async () => {
        zksyncContract.connect(wallet);
        const depositAmount = BigNumber.from('2');

        await zksyncContract.depositETH(wallet.address, { value: depositAmount });

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x01;
        let offset = 1;
        pubdata.writeUInt32BE(0xccaabbff, offset);
        offset += 4;
        pubdata.writeUInt32BE(0, offset); // token
        offset += 4;
        Buffer.from(
            depositAmount
                .toHexString()
                .substr(2)
                .padStart(16 * 2, '0'),
            'hex'
        ).copy(pubdata, offset);
        offset += 16;
        Buffer.from(wallet.address.substr(2), 'hex').copy(pubdata, offset);
        const blockData = newBlockDataFromPubdata(pubdata);
        blockData.onchainOperations.push({
            publicDataOffset: 0,
            ethWitness: '0x'
        });

        await zksyncContract.collectOnchainOpsExternal(blockData, EMPTY_KECCAK, 1, [1, 0, 0, 0, 0, 0]);
        await zksyncContract.commitPriorityRequests();
    });

    it('Process partial exit', async () => {
        zksyncContract.connect(wallet);
        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x03;
        const blockData = newBlockDataFromPubdata(pubdata);
        blockData.onchainOperations.push({
            publicDataOffset: 0,
            ethWitness: '0x'
        });

        const expectedHash = keccak256(ethers.utils.concat([EMPTY_KECCAK, pubdata]));
        await zksyncContract.collectOnchainOpsExternal(blockData, expectedHash, 0, [1, 0, 0, 0, 0, 0]);
    });

    it('Process full exit', async () => {
        zksyncContract.connect(wallet);
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);
        const fullExitAmount = parseEther('0.7');
        const accountId = 0x00faffaf;
        const serialId = 0;
        const contentHash = '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996';

        await zksyncContract.requestFullExit(accountId, tokenContract.address);
        // construct full exit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 11, 0);
        pubdata[0] = 0x06;
        let offset = 1;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(wallet.address.substr(2), 'hex').copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt32BE(tokenId, offset);
        offset += 4;
        Buffer.from(
            fullExitAmount
                .toHexString()
                .substr(2)
                .padStart(16 * 2, '0'),
            'hex'
        ).copy(pubdata, offset);
        offset += 16;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(wallet.address.substr(2), 'hex').copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt32BE(serialId, offset);
        offset += 4;
        Buffer.from(contentHash.substr(2), 'hex').copy(pubdata, offset);
        const blockData = newBlockDataFromPubdata(pubdata);
        blockData.onchainOperations.push({
            publicDataOffset: 0,
            ethWitness: '0x'
        });

        const expectedHash = keccak256(ethers.utils.concat([EMPTY_KECCAK, pubdata]));
        await zksyncContract.collectOnchainOpsExternal(blockData, expectedHash, 1, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        await zksyncContract.commitPriorityRequests();
    });

    it('Change pubkey with auth', async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = '0xfefefefefefefefefefefefefefefefefefefefe';
        await zksyncContract.setAuthPubkeyHash(pubkeyHash, nonce);

        const accountId = 0xffee12cc;

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x07;
        let offset = 1;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(pubkeyHash.substr(2), 'hex').copy(pubdata, offset);
        offset += 20;
        Buffer.from(wallet.address.substr(2), 'hex').copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt32BE(nonce, offset);

        const blockData = newBlockDataFromPubdata(pubdata);
        blockData.onchainOperations.push({
            publicDataOffset: 0,
            ethWitness: '0x'
        });

        await zksyncContract.collectOnchainOpsExternal(blockData, EMPTY_KECCAK, 0, [1, 0, 0, 0, 0, 0]);
    });

    it('Change pubkey with posted signature', async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = 'sync:fefefefefefefefefefefefefefefefefefefefe';
        const accountId = 0x00ffee12;
        const _ethWitness = await wallet.signMessage(zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId));
        const ethWitnessBytes = Uint8Array.from(Buffer.from(_ethWitness.slice(2), 'hex'));
        const ethWitness = ethers.utils.concat(['0x00', ethWitnessBytes, new Uint8Array(32).fill(0)]);

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x07;
        let offset = 1;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(pubkeyHash.substr(5), 'hex').copy(pubdata, offset);
        offset += 20;
        Buffer.from(wallet.address.substr(2), 'hex').copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt32BE(nonce, offset);
        const blockData = newBlockDataFromPubdata(pubdata);
        blockData.onchainOperations.push({
            publicDataOffset: 0,
            ethWitness: ethWitness
        });

        await zksyncContract.collectOnchainOpsExternal(blockData, EMPTY_KECCAK, 0, [1, 0, 0, 0, 0, 0]);
    });

    it('Process forced exit', async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.getTotalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x08;
        const blockData = newBlockDataFromPubdata(pubdata);
        blockData.onchainOperations.push({
            publicDataOffset: 0,
            ethWitness: '0x'
        });

        const expectedHash = keccak256(ethers.utils.concat([EMPTY_KECCAK, pubdata]));
        await zksyncContract.collectOnchainOpsExternal(blockData, expectedHash, 0, [1, 0, 0, 0, 0, 0]);

        const committedPriorityRequestsAfter = await zksyncContract.getTotalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, 'priority request number').eq(committedPriorityRequestsBefore);
    });
});
