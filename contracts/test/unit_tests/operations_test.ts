import { OperationsTest, OperationsTestFactory } from '../../typechain';
import { ethers } from 'ethers';

const hardhat = require('hardhat');

function getDepositPubdata({ accountId, tokenId, amount, owner }) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x01'),
        ethers.utils.arrayify(accountId),
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(amount),
        ethers.utils.arrayify(owner),
        ethers.utils.arrayify('0x0000000000000000000000')
    ]);
}

function getDepositPriorityQueueData({ tokenId, amount, owner }) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x01'),
        ethers.utils.arrayify('0x00000000'),
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(amount),
        ethers.utils.arrayify(owner)
    ]);
}

function getFullExitPubdata({
    accountId,
    tokenId,
    amount,
    owner,
    nftCreatorAccountId,
    nftCreatorAddress,
    nftSerialId,
    nftContentHash
}) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x06'),
        ethers.utils.arrayify(accountId),
        ethers.utils.arrayify(owner),
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(amount),
        ethers.utils.arrayify(nftCreatorAccountId),
        ethers.utils.arrayify(nftCreatorAddress),
        ethers.utils.arrayify(nftSerialId),
        ethers.utils.arrayify(nftContentHash),
        ethers.utils.arrayify('0x0000') // padding
    ]);
}

function getFullExitPriorityQueueData({ accountId, tokenId, owner }) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x06'),
        ethers.utils.arrayify(accountId),
        ethers.utils.arrayify(owner),
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(
            '0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000'
        ) // padding
    ]);
}

function getWithdrawPubdata({ accountId, tokenId, amount, packedFee, owner }) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x03'),
        ethers.utils.arrayify(accountId), // account id
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(amount),
        ethers.utils.arrayify(packedFee),
        ethers.utils.arrayify(owner),
        ethers.utils.arrayify('0x000000000000000000') // padding
    ]);
}

function getForcedExitPubdata({ initiatorAccountId, accountId, tokenId, amount, packedFee, owner }) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x08'),
        ethers.utils.arrayify(initiatorAccountId),
        ethers.utils.arrayify(accountId), // account id
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(amount),
        ethers.utils.arrayify(packedFee),
        ethers.utils.arrayify(owner),
        ethers.utils.arrayify('0x0000000000') // padding
    ]);
}

function getChangePubkeyPubdata({ accountId, pubKeyHash, owner, nonce, tokenId, packedFee }) {
    return ethers.utils.concat([
        ethers.utils.arrayify('0x07'),
        ethers.utils.arrayify(accountId),
        ethers.utils.arrayify(pubKeyHash), // account id
        ethers.utils.arrayify(owner),
        ethers.utils.arrayify(nonce),
        ethers.utils.arrayify(tokenId),
        ethers.utils.arrayify(packedFee),
        ethers.utils.arrayify('0x00') // padding
    ]);
}

describe('Operations unit tests', function () {
    this.timeout(50000);

    let testContract: OperationsTest;
    before(async () => {
        const contractFactory = await hardhat.ethers.getContractFactory('OperationsTest');
        const contract = await contractFactory.deploy();
        testContract = OperationsTestFactory.connect(contract.address, contract.signer);
    });

    it('Correctly Parse Deposit pubdata', async () => {
        const accountId = '0x01020304';
        const tokenId = '0x01020304';
        const amount = '0x101112131415161718191a1b1c1d1e1f';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';

        const pubdata = getDepositPubdata({ accountId, tokenId, amount, owner });
        await testContract.testDepositPubdata({ accountId, tokenId, amount, owner }, pubdata);
    });

    it('Correctly write Deposit data priority queue', async () => {
        const accountId = '0x01020304';
        const tokenId = '0x01020304';
        const amount = '0x101112131415161718191a1b1c1d1e1f';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';

        const priorityQueueData = getDepositPriorityQueueData({ tokenId, amount, owner });
        await testContract.testDepositPriorityQueue({ accountId, tokenId, amount, owner }, priorityQueueData);
    });

    it('Correctly Parse FullExit pubdata', async () => {
        const accountId = '0x01020304';
        const nftCreatorAccountId = '0x01020304';
        const nftSerialId = '0x01020304';
        const tokenId = '0x01020304';
        const amount = '0x101112131415161718191a1b1c1d1e1f';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const nftCreatorAddress = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const nftContentHash = '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996';

        const pubdata = getFullExitPubdata({
            accountId,
            tokenId,
            amount,
            owner,
            nftCreatorAccountId,
            nftCreatorAddress,
            nftSerialId,
            nftContentHash
        });
        await testContract.testFullExitPubdata(
            { accountId, tokenId, amount, owner, nftCreatorAccountId, nftCreatorAddress, nftSerialId, nftContentHash },
            pubdata
        );
    });

    it('Correctly Write FullExit data priority queue', async () => {
        const accountId = '0x01020304';
        const tokenId = '0x01020304';
        const nftCreatorAccountId = '0x01020304';
        const nftSerialId = '0x01020304';
        const amount = '0x101112131415161718191a1b1c1d1e1f';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const nftCreatorAddress = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const nftContentHash = '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996';

        const priorityQueueData = getFullExitPriorityQueueData({ accountId, tokenId, owner });

        await testContract.testFullExitPriorityQueue(
            { accountId, tokenId, amount, owner, nftCreatorAccountId, nftCreatorAddress, nftSerialId, nftContentHash },
            priorityQueueData
        );
    });

    it('Correctly Parse Withdraw pubdata', async () => {
        const tokenId = '0x01020304';
        const accountId = '0x01020304';
        const amount = '0x101112131415161718191a1b1c1d1e1f';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const packedFee = '0xffee';

        const pubdata = getWithdrawPubdata({ accountId, tokenId, amount, packedFee, owner });
        await testContract.testWithdrawPubdata({ tokenId, amount, owner }, pubdata);
    });

    it('Correctly Parse ForcedExit pubdata', async () => {
        const tokenId = '0x01020304';
        const initiatorAccountId = '0xa1a2a3a4';
        const accountId = '0x01020304';
        const amount = '0x101112131415161718191a1b1c1d1e1f';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const packedFee = '0xffee';

        const pubdata = getForcedExitPubdata({ initiatorAccountId, accountId, tokenId, amount, packedFee, owner });
        await testContract.testForcedExitPubdata({ tokenId, amount, target: owner }, pubdata);
    });

    it('Correctly Parse ChangePubKey pubdata', async () => {
        const accountId = '0x01020304';
        const pubKeyHash = '0x4f6C02876350d615be18C530D869cF746D69d1df';
        const owner = '0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5';
        const nonce = '0xa1a2a3a4';
        const tokenId = '0x01020304';
        const packedFee = '0xffee';

        const pubdata = getChangePubkeyPubdata({ accountId, pubKeyHash, owner, nonce, tokenId, packedFee });
        await testContract.testChangePubkeyPubdata({ accountId, pubKeyHash, owner, nonce }, pubdata);
    });
});
