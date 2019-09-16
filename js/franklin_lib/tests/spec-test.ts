import {musigPedersen, musigSHA256, privateKeyToPublicKey} from "../src/sign";
import BN = require("bn.js");
import Axios from 'axios';
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

}

async function main() {
    // await signatureCheck();
    await addressCheck();
}

main();