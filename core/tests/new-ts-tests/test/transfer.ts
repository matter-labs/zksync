import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<BigNumber>;
        // testMultiTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<BigNumber>;
        // testFailedMultiTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<BigNumber>;
    }
}

Tester.prototype.testTransfer = async function(
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber,
) {
    const fullFee = await this.syncProvider.getTransactionFee("Transfer", receiver.address(), token);
    const fee = fullFee.totalFee;
    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);

    const transferToNewHandle = await sender.syncTransfer({
        to: receiver.address(),
        token,
        amount,
        fee,
    });

    // await sleep(timeoutBeforeReceipt);
    await transferToNewHandle.awaitReceipt();
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    
    if (sender.address() === receiver.address()) {
        expect(senderBefore.sub(fee).eq(senderAfter), "Transfer to self checks failed").to.be.true;
    } else {
        expect(senderBefore.sub(senderAfter).eq(amount.add(fee)), "Transfer checks failed").to.be.true;
        expect(receiverAfter.sub(receiverBefore).eq(amount), "Transfer checks failed").to.be.true;
    }

    return fee;
}

// Tester.prototype.testMultiTransfer = async function(
//     sender: Wallet,
//     receiver: Wallet,
//     token: TokenLike,
//     amount: BigNumber,
// ) {
//     const fee = await this.syncProvider.getTransactionsBatchFee(
//         ["Transfer", "Transfer"],
//         [receiver.address(), receiver.address()],
//         token
//     );
//
//     // First, execute batched transfers successfully.
//
//     {
//         const senderBefore = await sender.getBalance(token);
//         const receiverBefore = await receiver.getBalance(token);
//         const tx = {
//             to: receiver.address(),
//             token,
//             amount,
//             fee: fee.div(2),
//         }
//         const handles = await sender.syncMultiTransfer([
//             Object.assign(tx), Object.assign(tx)
//         ]);
//         await Promise.all(handles.map(handle => handle.awaitReceipt()));
//         const senderAfter = await sender.getBalance(token);
//         const receiverAfter = await receiver.getBalance(token);
//         expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(fee)), "Batched transfer checks failed").to.be.true;
//         expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2).add(fee)), "Batched transfer checks failed").to.be.true;
//     }
//
//     // Then, send another batch in which the second transaction will fail.
//     // The first transaction should not be executed.
//
//     {
//         const wallet1BeforeTransfer = await sender.getBalance(token);
//         const wallet2BeforeTransfer = await receiver.getBalance(token);
//         const transferHandles = await sender.syncMultiTransfer([
//             {
//                 to: receiver.address(),
//                 token,
//                 amount,
//                 fee: fee.div(2),
//             },
//             {
//                 to: receiver.address(),
//                 token,
//                 amount: amount.mul(10000), // Set too big amount for the 2nd transaction.
//                 fee: fee.div(2),
//             },
//         ]);
//         for (let i = 0; i < transferHandles.length; i++) {
//             try {
//                 await transferHandles[i].awaitReceipt();
//             } catch (e) {
//                 console.log("Error (expected) on sync tx fail:", e.message);
//             }
//         }
//         const wallet1AfterTransfer = await sender.getBalance(token);
//         const wallet2AfterTransfer = await receiver.getBalance(token);
//
//         let transferCorrect = true;
//         transferCorrect = transferCorrect && wallet1BeforeTransfer.eq(wallet1AfterTransfer);
//         transferCorrect = transferCorrect && wallet2AfterTransfer.eq(wallet2BeforeTransfer);
//         if (!transferCorrect) {
//             throw new Error("Batched transfer checks failed: balances changed after batch failure");
//         }
//     }
//
//     return fee;
// }
//
