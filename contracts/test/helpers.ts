
import { utils as syncutils } from "zksync";
import { BN } from "bn.js";
import { expect, use } from "chai";
import { solidity } from "ethereum-waffle";
import { bigNumberify } from "ethers/utils";

use(solidity);

export async function cancelOustandingDepositsForExodus(
    provider,
    wallet,
    franklinDeployedContract,
    priorityQueueDeployedContract,
    expectedToCancel,
    actualToCancel,
    expectedBalanceToWithdraw,
    revertCode,
) {
    if (!revertCode) {
        const oldOpenPriorityRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
        const oldCommittedPriorityRequests = await priorityQueueDeployedContract.totalCommittedPriorityRequests();
        const oldFirstPriorityRequestId = await priorityQueueDeployedContract.firstPriorityRequestId();

        const tx = await franklinDeployedContract.cancelOutstandingDepositsForExodusMode(expectedToCancel);
        await tx.wait();

        const newOpenPriorityRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
        const newCommittedPriorityRequests = await priorityQueueDeployedContract.totalCommittedPriorityRequests();
        const newFirstPriorityRequestId = await priorityQueueDeployedContract.firstPriorityRequestId();

        expect(oldOpenPriorityRequests - newOpenPriorityRequests).equal(actualToCancel);
        expect(oldCommittedPriorityRequests - newCommittedPriorityRequests).equal(0);
        expect(newFirstPriorityRequestId - oldFirstPriorityRequestId).equal(actualToCancel);

        const balanceToWithdraw = await franklinDeployedContract.balancesToWithdraw(wallet.address, 0);
        expect(balanceToWithdraw).equal(expectedBalanceToWithdraw);
    } else {
        const tx = await franklinDeployedContract.cancelOutstandingDepositsForExodusMode(
            expectedToCancel,
            { gasLimit: bigNumberify("6000000") },
        );
        await tx.wait()
            .catch(() => { });

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));

        expect(reason.substring(0, 5)).equal(revertCode);
    }
}

export async function postEthDeposit(
    provider,
    wallet,
    franklinDeployedContract,
    priorityQueueDeployedContract,
    depositAmount,
    fee,
    franklinAddress,
    txValue,
    revertCode,
) {
    const franklinAddressBinary = Buffer.from(franklinAddress, "hex");

    const oldOpenPriorityRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
    const oldCommittedPriorityRequests = await priorityQueueDeployedContract.totalCommittedPriorityRequests();
    const oldFirstPriorityRequestId = await priorityQueueDeployedContract.firstPriorityRequestId();

    const tx = await franklinDeployedContract.depositETH(
        depositAmount,
        franklinAddressBinary,
        { value: txValue, gasLimit: bigNumberify("6000000") },
    );

    if (!revertCode) {
        const receipt = await tx.wait();
        const event = receipt.events[1].args;

        expect(event.owner).equal(wallet.address);
        expect(event.tokenId).equal(0);
        expect(event.amount).equal(depositAmount);
        // FIXME: not passing: expect(event.fee).equal(fee);

        expect(event.franklinAddress).equal("0x" + franklinAddress);

        const newOpenPriorityRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
        const newCommittedPriorityRequests = await priorityQueueDeployedContract.totalCommittedPriorityRequests();
        const newFirstPriorityRequestId = await priorityQueueDeployedContract.firstPriorityRequestId();

        expect(newOpenPriorityRequests - oldOpenPriorityRequests).equal(1);
        expect(newCommittedPriorityRequests - oldCommittedPriorityRequests).equal(0);
        expect(newFirstPriorityRequestId - oldFirstPriorityRequestId).equal(0);

    } else {
        await tx.wait()
            .catch(() => { });

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));

        expect(reason.substring(0, 5)).equal(revertCode);
    }
}

export async function postErc20Deposit(
    provider,
    wallet,
    franklinDeployedContract,
    priorityQueueDeployedContract,
    token,
    depositAmount,
    fee,
    franklinAddress,
    txValue,
    revertCode,
) {
    await token.approve(franklinDeployedContract.address, depositAmount);

    const franklinAddressBinary = Buffer.from(franklinAddress, "hex");

    const oldOpenPriorityRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
    const oldCommittedPriorityRequests = await priorityQueueDeployedContract.totalCommittedPriorityRequests();
    const oldFirstPriorityRequestId = await priorityQueueDeployedContract.firstPriorityRequestId();

    const tx = await franklinDeployedContract.depositERC20(
        token.address,
        depositAmount,
        franklinAddressBinary,
        { value: txValue, gasLimit: bigNumberify("6000000") },
    );

    if (!revertCode) {
        const receipt = await tx.wait();
        const event = receipt.events[3].args;

        expect(event.owner).equal(wallet.address);
        expect(event.amount).equal(depositAmount);
        //FIXME: expect(event.fee).equal(fee);
        expect(event.franklinAddress).equal("0x" + franklinAddress);

        const newOpenPriorityRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
        const newCommittedPriorityRequests = await priorityQueueDeployedContract.totalCommittedPriorityRequests();
        const newFirstPriorityRequestId = await priorityQueueDeployedContract.firstPriorityRequestId();

        expect(newOpenPriorityRequests - oldOpenPriorityRequests).equal(1);
        expect(newCommittedPriorityRequests - oldCommittedPriorityRequests).equal(0);
        expect(newFirstPriorityRequestId - oldFirstPriorityRequestId).equal(0);

        //console.log("Posted new deposit");
    } else {
        await tx.wait()
            .catch(() => { });

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));

        expect(reason.substring(0, 5)).equal(revertCode);
    }
}

export async function postBlockCommit(
    provider,
    wallet,
    franklinDeployedContract,
    blockNumber,
    feeAcc,
    newRoot,
    pubData,
    onchainOperationsNumber,
    priorityOperationsNumber,
    commitment,
    revertCode,
) {
    const root = Buffer.from(newRoot, "hex");
    const tx = await franklinDeployedContract.commitBlock(
        blockNumber,
        feeAcc,
        root,
        pubData,
        {
            gasLimit: bigNumberify("500000"),
        },
    );
    if (!revertCode) {
        
        const commitReceipt = await tx.wait();
        const commitEvents = commitReceipt.events;

        const commitedEvent1 = commitEvents[0].args;

        expect(commitedEvent1.blockNumber).equal(blockNumber);

        expect((await franklinDeployedContract.blocks(blockNumber)).onchainOperations).equal(onchainOperationsNumber);
        expect((await franklinDeployedContract.blocks(blockNumber)).priorityOperations).equal(priorityOperationsNumber);

        //FIXME: why is this failing on ganache?
        //expect((await franklinDeployedContract.blocks(blockNumber)).commitment).equal(commitment);

        expect((await franklinDeployedContract.blocks(blockNumber)).stateRoot).equal("0x" + newRoot);
        expect((await franklinDeployedContract.blocks(blockNumber)).validator).equal(wallet.address);

    } else {
        await tx.wait()
            .catch(() => { });

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));
        expect(reason.substring(0, 5)).equal(revertCode);
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
        { gasLimit: bigNumberify("500000") },
    );
    if (!revertCode) {
        const receipt = await tx.wait();
        const events = receipt.events;

        const event = events.pop().args;

        expect(event.blockNumber).equal(blockNumber);
    } else {
        await tx.wait()
            .catch(() => { });

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
    const exitTx = await franklinDeployedContract.withdrawETH(balanceToWithdraw, {
        gasLimit: bigNumberify("6000000"),
    });
    if (!revertCode) {
        const exitTxReceipt = await exitTx.wait();
        const gasUsed = exitTxReceipt.gasUsed.mul(await provider.getGasPrice());
        const newBalance = await wallet.getBalance();
        expect(newBalance.sub(oldBalance).add(gasUsed)).eq(balanceToWithdraw);
    } else {
        await exitTx.wait()
            .catch(() => { });

        const code = await provider.call(exitTx, exitTx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));
        expect(reason.substring(0, 5)).equal(revertCode);
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
    const rollupBalance = await franklinDeployedContract.balancesToWithdraw(wallet.address, tokenId);
    const oldBalance = await token.balanceOf(wallet.address);
    const exitTx = await franklinDeployedContract.withdrawERC20(
        token.address,
        balanceToWithdraw,
        { gasLimit: bigNumberify("500000") },
    );
    if (!revertCode) {
        await exitTx.wait();
        const newBalance = await token.balanceOf(wallet.address);
        const newRollupBalance = await franklinDeployedContract.balancesToWithdraw(wallet.address, tokenId);
        expect(rollupBalance - newRollupBalance).equal(bigNumberify(balanceToWithdraw));
        expect(newBalance.sub(oldBalance)).eq(balanceToWithdraw);
    } else {
        await exitTx.wait()
            .catch(() => { });

        const code = await provider.call(exitTx, exitTx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));
        expect(reason.substring(0, 5)).equal(revertCode);
    }
}

export async function postFullExit(
    provider,
    franklinDeployedContract,
    priorityQueueDeployedContract,
    accountId,
    pubKey,
    tokenAddress,
    signature,
    nonce,
    value,
    revertCode,
) {
    const sig = Buffer.from(signature, "hex");
    const beforeTotalOpenRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
    const tx = await franklinDeployedContract.fullExit(
        accountId,
        pubKey,
        tokenAddress,
        sig,
        nonce,
        {
            value,
            gasLimit: bigNumberify("500000"),
        },
    );
    if (!revertCode) {
        await tx.wait();
        const afterTotalOpenRequests = await priorityQueueDeployedContract.totalOpenPriorityRequests();
        expect(afterTotalOpenRequests - beforeTotalOpenRequests).equal(1);
    } else {
        await tx.wait()
            .catch(() => { });

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));
        expect(reason.substring(0, 5)).equal(revertCode);
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
    const pubkeyBytes = Buffer.alloc(32, 0);
    if (ethAddress.startsWith("0x")) {
        ethAddress = ethAddress.substr(2);
    }
    const addressBytes = Buffer.from(ethAddress, "hex");
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    const nonceBytes = Buffer.alloc(4, 0);
    const signatureBytes = Buffer.alloc(64, 0);
    if (hexAmount.startsWith("0x")) {
        hexAmount = hexAmount.substr(2);
    }
    const amountBytes = Buffer.from(hexAmount, "hex");
    const pad1BytesLength = 16 - amountBytes.length;
    const pad1Bytes = Buffer.alloc(pad1BytesLength, 0);
    const pad2Bytes = Buffer.alloc(2, 0);

    return Buffer.concat([
        txId,
        accountId,
        pubkeyBytes,
        addressBytes,
        tokenBytes,
        nonceBytes,
        signatureBytes,
        pad1Bytes,
        amountBytes,
        pad2Bytes,
    ]);
}

export function createNoopPublicData(): Buffer {
    const txId = Buffer.from("00", "hex");
    const padBytes = Buffer.alloc(7, 0);

    return Buffer.concat([txId, padBytes]);
}

export function createWrongNoopPublicData(): Buffer {
    const txId = Buffer.from("00", "hex");
    const padBytes = Buffer.alloc(6, 0);

    return Buffer.concat([txId, padBytes]);
}

export function createWrongOperationPublicData(): Buffer {
    const txId = Buffer.from("07", "hex");
    const padBytes = Buffer.alloc(7, 0);

    return Buffer.concat([txId, padBytes]);
}

export function hex_to_ascii(str1) {
    const hex = str1.toString();
    let str = "";
    for (let n = 0; n < hex.length; n += 2) {
        str += String.fromCharCode(parseInt(hex.substr(n, 2), 16));
    }
    return str;
}
