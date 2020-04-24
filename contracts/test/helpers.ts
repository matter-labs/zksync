import {utils as syncutils} from "zksync";
import * as BN from "bn.js";
import {expect, use} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify} from "ethers/utils";

use(solidity);

export const OPERATIONS = {
    changePubkeyOnchain: {
        chunks: 6,
        id: 0x08,
    },
    deposit: {
        chunks: 6,
        id: 0x01,
    },
    fullExit: {
        chunks: 6,
        id: 0x06,
    },
    noop: {
        chunks: 1,
        id: 0x00,
    },
    withdraw: {
        chunks: 6,
        id: 0x03,
    },
};
export const CHUNKS_SIZE = 8;

export async function cancelOustandingDepositsForExodus(
    provider,
    wallet,
    franklinDeployedContract,
    expectedToCancel,
    actualToCancel,
    expectedBalanceToWithdraw,
    revertCode,
) {
    const oldOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
    const oldCommittedPriorityRequests = await franklinDeployedContract.totalCommittedPriorityRequests();
    const oldFirstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();

    const receipt = await executeTransaction(async () => {
        return await franklinDeployedContract.cancelOutstandingDepositsForExodusMode(expectedToCancel, { gasLimit: 1000000 });
    }, revertCode);

    if (receipt) {
        const newOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        const newCommittedPriorityRequests = await franklinDeployedContract.totalCommittedPriorityRequests();
        const newFirstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();

        expect(oldOpenPriorityRequests - newOpenPriorityRequests).equal(actualToCancel);
        expect(oldCommittedPriorityRequests - newCommittedPriorityRequests).equal(0);
        expect(newFirstPriorityRequestId - oldFirstPriorityRequestId).equal(actualToCancel);

        const balanceToWithdraw = (await franklinDeployedContract.balancesToWithdraw(wallet.address, 0)).balanceToWithdraw;
        expect(balanceToWithdraw, "Exodus mode deposit cancel balance mismatch").equal(expectedBalanceToWithdraw);
    }
}

async function executeTransaction(txClosure, revertCode) {
    let receipt;
    try {
        const tx = await txClosure();
        receipt = await tx.wait();
        console.log('revert code:', revertCode, revertCode && revertCode.length)
        if (revertCode) {
            expect(receipt.status, `expected transaction fail with code: ${revertCode}`).not.eq(1);
        }
    } catch (e) {
        if (e.name === "RuntimeError") {
            const txHash = e.transactionHash;
            const revertReason = e.results[txHash].reason;

            expect(revertReason, `expected transaction revert`).eq(revertCode);
        } else {
            throw e;
        }
    }
    return receipt;
}

export async function postEthDeposit(
    provider,
    wallet,
    franklinDeployedContract,
    depositAmount,
    fee,
    franklinAddress,
    txValue,
    revertCode,
) {
    const receipt = await executeTransaction(async () => {
        return franklinDeployedContract.depositETH(
            depositAmount,
            franklinAddress,
            {value: txValue, gasLimit: bigNumberify("6000000")},
        );
    }, revertCode);
    if (receipt) {
        const event = receipt.events[1].args;
        expect(event.owner).equal(franklinAddress);
        expect(event.tokenId).equal(0);
        expect(event.amount).equal(depositAmount);
    }
}

export async function postErc20Deposit(
    provider,
    wallet,
    franklinDeployedContract,
    token,
    depositAmount,
    fee,
    franklinAddress,
    txValue,
    revertCode,
) {
    await token.approve(franklinDeployedContract.address, depositAmount);

    const receipt = await executeTransaction(async () => {
        return franklinDeployedContract.depositERC20(
            token.address,
            depositAmount,
            franklinAddress,
            {value: txValue, gasLimit: bigNumberify("6000000")},
        );
    }, revertCode);
    if (receipt) {
        const event = receipt.events[1].args;
        expect(event.owner).equal(franklinAddress);
        expect(event.amount).equal(depositAmount);
    }
}

export async function postBlockCommit(
    wallet,
    franklinDeployedContract,
    commitArgs: {
        blockNumber,
        feeAcc,
        newRoot,
        pubData,
        witnessData,
        witnessSizes,
    },
    onchainOperationsNumber,
    priorityOperationsNumber,
    commitment,
    revertCode: string,
    triggerExodus = false,
) {
    const root = Buffer.from(commitArgs.newRoot, "hex");
    const receipt = await executeTransaction(async () => {
        return await franklinDeployedContract.commitBlock(
            commitArgs.blockNumber,
            commitArgs.feeAcc,
            root,
            commitArgs.pubData,
            commitArgs.witnessData, commitArgs.witnessSizes,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
    }, revertCode);
    if (receipt) {
        const commitEvents = receipt.events;
        const commitedEvent1 = commitEvents[0];

        if (!triggerExodus) {
            // expect(commitedEvent1.args.blockNumber).equal(blockNumber);
            // expect((await franklinDeployedContract.blocks(blockNumber)).onchainOperations).equal(onchainOperationsNumber);
            // expect((await franklinDeployedContract.blocks(blockNumber)).priorityOperations).equal(priorityOperationsNumber);
            // //FIXME: why is this failing on ganache?
            // //expect((await franklinDeployedContract.blocks(blockNumber)).commitment).equal(commitment);
            // expect((await franklinDeployedContract.blocks(blockNumber)).stateRoot).equal("0x" + newRoot);
            // expect((await franklinDeployedContract.blocks(blockNumber)).validator).equal(wallet.address);
        } else {
            expect(commitedEvent1.event, "ExodusEvent expected").eq("ExodusMode");
        }
    }
}

export async function postBlockVerify(
    provider,
    franklinDeployedContract,
    blockNumber,
    proof,
    revertCode,
) {
    const tx = await franklinDeployedContract.verifyBlock(
        blockNumber,
        proof,
        {gasLimit: bigNumberify("500000")},
    );
    if (!revertCode) {
        const receipt = await tx.wait();
        const events = receipt.events;

        const event = events.pop().args;

        expect(event.blockNumber).equal(blockNumber);
    } else {
        const receipt = await tx.wait()
            .catch(() => {
            });
        if (receipt && receipt.status) {
            expect(receipt.status, `tx should fail ${receipt.transactionHash}`).not.eq(1);
        }

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));
        expect(reason.substring(0, 5)).equal(revertCode);
    }
}

export async function withdrawEthFromContract(
    provider,
    wallet,
    franklinDeployedContract,
    balanceToWithdraw,
    revertCode,
) {
    const oldBalance = await wallet.getBalance();
    const receipt = await executeTransaction(async () => {
        return  franklinDeployedContract.withdrawETH(balanceToWithdraw, {
            gasLimit: bigNumberify("6000000"),
        });
    }, revertCode);
    if (receipt) {
        const gasUsed = receipt.gasUsed.mul(await provider.getGasPrice());
        const newBalance = await wallet.getBalance();
        expect(newBalance.sub(oldBalance).add(gasUsed)).eq(balanceToWithdraw);
    }
}

export async function withdrawErcFromContract(
    provider,
    wallet,
    franklinDeployedContract,
    token,
    tokenId,
    balanceToWithdraw,
    revertCode,
) {
    const rollupBalance = (await franklinDeployedContract.balancesToWithdraw(wallet.address, tokenId)).balanceToWithdraw;
    const oldBalance = await token.balanceOf(wallet.address);


    const receipt = await executeTransaction(async () => {
        return franklinDeployedContract.withdrawERC20(
            token.address,
            balanceToWithdraw,
            {gasLimit: bigNumberify("500000")},
        );
    }, revertCode);
    if (receipt) {
        const newBalance = await token.balanceOf(wallet.address);
        const newRollupBalance = (await franklinDeployedContract.balancesToWithdraw(wallet.address, tokenId)).balanceToWithdraw;
        expect(rollupBalance - newRollupBalance).equal(bigNumberify(balanceToWithdraw));
        expect(newBalance.sub(oldBalance)).eq(balanceToWithdraw);
    }
}

export async function postFullExit(
    provider,
    franklinDeployedContract,
    accountId,
    tokenAddress,
    value,
    revertCode,
) {
    const beforeTotalOpenRequests = await franklinDeployedContract.totalOpenPriorityRequests();
    const receipt = await executeTransaction(async () => {
        return await franklinDeployedContract.fullExit(
            accountId,
            tokenAddress,
            {
                gasLimit: bigNumberify("500000"),
                value,
            },
        );
    }, revertCode);
    if (receipt) {
        const afterTotalOpenRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        expect(afterTotalOpenRequests - beforeTotalOpenRequests).equal(1);
    }
}

export function createDepositPublicData(tokenId, hexAmount: string, franklinAddress: string): Buffer {
    const txId = Buffer.from("01", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(2, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    if (hexAmount.startsWith("0x")) {
        hexAmount = hexAmount.substr(2);
    }
    const amountBytes = Buffer.from(hexAmount, "hex");
    const pad1BytesLength = 16 - amountBytes.length;
    const pad1Bytes = Buffer.alloc(pad1BytesLength, 0);
    if (franklinAddress.startsWith("0x")) {
        franklinAddress = franklinAddress.substr(2);
    }
    const addressBytes = Buffer.from(franklinAddress, "hex");
    const pad2Bytes = Buffer.alloc(6, 0);

    return Buffer.concat([txId, accountId, tokenBytes, pad1Bytes, amountBytes, addressBytes, pad2Bytes]);
}

export function createWrongDepositPublicData(tokenId, hexAmount: string, franklinAddress: string): Buffer {
    const txId = Buffer.from("01", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(2, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    if (hexAmount.startsWith("0x")) {
        hexAmount = hexAmount.substr(2);
    }
    const amountBytes = Buffer.from(hexAmount, "hex");
    const pad1BytesLength = 14 - amountBytes.length;
    const pad1Bytes = Buffer.alloc(pad1BytesLength, 0);
    if (franklinAddress.startsWith("0x")) {
        franklinAddress = franklinAddress.substr(2);
    }
    const addressBytes = Buffer.from(franklinAddress, "hex");

    return Buffer.concat([txId, accountId, tokenBytes, pad1Bytes, amountBytes, addressBytes]);
}

export function createWithdrawPublicData(tokenId, hexAmount: string, ethAddress: string): Buffer {
    const txId = Buffer.from("03", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(2, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    if (hexAmount.startsWith("0x")) {
        hexAmount = hexAmount.substr(2);
    }
    const amountBytes = Buffer.from(hexAmount, "hex");
    const pad1BytesLength = 16 - amountBytes.length;
    const pad1Bytes = Buffer.alloc(pad1BytesLength, 0);
    const feeBytes = syncutils.packFeeChecked(new BN("0"));
    if (ethAddress.startsWith("0x")) {
        ethAddress = ethAddress.substr(2);
    }
    const addressBytes = Buffer.from(ethAddress, "hex");
    const pad2Bytes = Buffer.alloc(4, 0);

    return Buffer.concat([txId, accountId, tokenBytes, pad1Bytes, amountBytes, feeBytes, addressBytes, pad2Bytes]);
}

export function createFullExitPublicData(accId, ethAddress: string, tokenId, hexAmount: string): Buffer {
    const txId = Buffer.from("06", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(accId, 0, 3);
    const addressBytes = Buffer.from(ethAddress.substr(2), "hex");
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    const amountBytes = Buffer.from(hexAmount.substr(2), "hex");
    const pad1BytesLength = 16 - amountBytes.length;
    const pad1Bytes = Buffer.alloc(pad1BytesLength, 0);

    const dataWithouPad = Buffer.concat([
        txId,
        accountId,
        addressBytes,
        tokenBytes,
        pad1Bytes,
        amountBytes,
    ]);
    const resultPubData = Buffer.alloc(6 * 8, 0);
    dataWithouPad.copy(resultPubData);

    return resultPubData;
}

export function createNoopPublicData(): Buffer {
    return Buffer.alloc(OPERATIONS.noop.chunks * CHUNKS_SIZE, 0);
}

export function createWrongNoopPublicData(): Buffer {
    return Buffer.alloc(OPERATIONS.noop.chunks * (CHUNKS_SIZE - 1), 0);
}

export function createWrongOperationPublicData(): Buffer {
    const pubdata = Buffer.alloc(CHUNKS_SIZE, 0);
    pubdata[0] = 0xff; // Unknown op type

    return pubdata;
}

export function hex_to_ascii(str1) {
    const hex = str1.toString();
    let str = "";
    for (let n = 0; n < hex.length; n += 2) {
        str += String.fromCharCode(parseInt(hex.substr(n, 2), 16));
    }
    return str;
}
