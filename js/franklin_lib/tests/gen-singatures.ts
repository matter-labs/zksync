import {musigPedersen, musigSHA256, privateKeyToPublicKey} from "../src/sign";
import BN = require("bn.js");

const crypto = require("crypto");


function main() {
    let testMessages = [ Buffer.from([])];

    for (let len = 1; len <= 2; ++len) {
        let msg = crypto.randomBytes(len);
        testMessages.push(msg);
    }

    const privKey = new BN(10);
    const result = {
        public_key: {
            x: privateKeyToPublicKey(privKey).getX().toString("hex"),
            y: privateKeyToPublicKey(privKey).getY().toString("hex")
        },
        messages: testMessages.map(msg => msg.toJSON().data),
        sha256_signatures: [],
        pedersen_signatures: []
    };
    for (let msg of testMessages) {
        result.sha256_signatures.push(musigSHA256(privKey, msg));
        result.pedersen_signatures.push(musigPedersen(privKey, msg));
    }

    console.log(JSON.stringify(result,null, 2));
}

main();