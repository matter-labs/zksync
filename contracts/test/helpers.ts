
import {BigNumber} from "ethers/utils";
import {packAmount, packFee} from "../../js/franklin_lib/src/utils";
import {BN} from "bn.js";

export function createDepositPublicData(tokenId, amount: BigNumber, franklinAddress: string): Buffer {
    const txId = Buffer.from("01", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(0, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    const amountBytes = packAmount(new BN(amount.toString()));
    const addressBytes = Buffer.from(franklinAddress, "hex");
    const padBytes = Buffer.alloc(3, 0);

    return Buffer.concat([txId, accountId, tokenBytes, amountBytes, addressBytes, padBytes]);
}

export function createPartialExitPublicData(tokenId, amount: BigNumber, ethAddress: string): Buffer {
    const txId = Buffer.from("03", "hex");
    const accountId = Buffer.alloc(3, 0);
    accountId.writeUIntBE(0, 0, 3);
    const tokenBytes = Buffer.alloc(2);
    tokenBytes.writeUInt16BE(tokenId, 0);
    const amountBytes = packAmount(new BN(amount.toString()));
    const feeBytes = packFee(new BN("0"));
    const addressBytes = Buffer.from(ethAddress.substr(2), "hex");
    const padBytes = Buffer.alloc(2, 0);

    return Buffer.concat([txId, accountId, tokenBytes, amountBytes, feeBytes, addressBytes, padBytes]);
}