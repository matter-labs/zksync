import {
    addressLen,
    musigPedersen,
    musigSHA256,
    privateKeyToPublicKey,
    pubkeyToAddress,
    serializePointPacked
} from "../src/crypto";
import BN = require("bn.js");
import Axios from 'axios';
import {Address, FranklinProvider, WalletKeys} from "../src/wallet";
import {ethers} from 'ethers';
import {packAmount, packFee} from "../src/utils";
const crypto = require("crypto");

const specTestServer = "http://127.0.0.1:8734";

async function signatureCheck() {
    for (let len = 1; len <= 32; ++len) {
        let msg = crypto.randomBytes(len);
        const privKey = new BN(crypto.randomBytes(16));
        msg.toJSON().data;
        for(let variant of ["MusigPedersen", "MusigSha256"]) {
            let signature = null;
            if (variant == "MusigPedersen") {
                signature = musigPedersen(privKey, msg);
            } else if (variant == "MusigSha256") {
                signature = musigSHA256(privKey, msg);
            }
            const req = {
                msg: msg.toJSON().data,
                signature,
                variant
            };
            let resp = await Axios.post(specTestServer + '/check_signature', req).then(reps => reps.data);
            if (!resp.correct) {
                throw {error: "Signature is not correct", req};
            }
        }
    }
}

async function addressCheck() {
    let keys = new WalletKeys(new BN(crypto.randomBytes(16)));

    let address = pubkeyToAddress(keys.publicKey);
    let resp = await Axios.post(specTestServer + '/address', {pub_key: serializePointPacked(keys.publicKey).toString("hex")}).then(reps => reps.data);
    let serverAddress = Buffer.from(resp.address.substr(2), "hex");
    if (!serverAddress.equals(address)) {
        throw "Pubkey to address mismatch";
    }
}

async function txSignatureCheck() {
    let keys = new WalletKeys(new BN(crypto.randomBytes(16)));
    let transfer = {
            from: pubkeyToAddress(keys.publicKey),
            to: crypto.randomBytes(addressLen),
            token: 2,
            amount: ethers.utils.parseEther("1000"),
            fee: ethers.utils.parseEther("100"),
            nonce: 17,
    };

    let transferSign = keys.signTransfer(transfer);
    let req = FranklinProvider.prepareTransferRequestForNode(transfer, transferSign);
    let resp = await Axios.post(specTestServer + '/check_tx_signature', req).then(reps => reps.data);
    if (!resp.valid) {
        throw "invalid transfer signature";
    }


    let withdraw = {
        account: pubkeyToAddress(keys.publicKey),
        eth_address: ethers.constants.AddressZero,
        token: 2,
        amount: 11,
        fee: 12,
        nonce: 17,
    };

    let withdrawSign = keys.signWithdraw(withdraw);
    req = FranklinProvider.prepareWithdrawRequestForNode(withdraw, withdrawSign);
    resp = await Axios.post(specTestServer + '/check_tx_signature', req).then(reps => reps.data);
    if (!resp.valid) {
        throw "invalid withdraw signature";
    }

    let close = {
        account: pubkeyToAddress(keys.publicKey),
        nonce: 17,
    };

    let closeSign = keys.signClose(close);
    req = FranklinProvider.prepareCloseRequestForNode(close, closeSign);
    resp = await Axios.post(specTestServer + '/check_tx_signature', req).then(reps => reps.data);
    if (!resp.valid) {
        throw "invalid close signature";
    }
}

async function fullExitSignatureCheck() {
    let keys = new WalletKeys(new BN(crypto.randomBytes(16)));
    let token = 22;
    let nonce = 78;
    let ethAddress = ethers.constants.AddressZero;
    let sign = keys.signFullExit({token, eth_address: ethAddress, nonce});
    let req = {
        packed_pubkey: serializePointPacked(keys.publicKey).toJSON().data,
        eth_address: ethAddress,
        token,
        nonce,
        signature_r: sign.slice(0, 32).toJSON().data,
        signature_s: sign.slice(32, 64).toJSON().data
    };
    let resp = await Axios.post(specTestServer + '/check_full_exit_signature', req).then(reps => reps.data);
    if (!resp.valid) {
        throw "Invalid full exit signature";
    }

    if (!Buffer.from(resp.account_address.substr(2), "hex").equals( pubkeyToAddress(keys.publicKey))) {
        throw "Invalid signer address";
    }
}

async function main() {
    await signatureCheck();
    await addressCheck();
    await txSignatureCheck();
    await fullExitSignatureCheck();
}

main();