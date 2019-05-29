const elliptic = require('elliptic');
const BN = require("bn.js");
const assert = require("assert");
const Buffer = require('buffer/').Buffer  // note: the trailing slash is important!

//! `Fr modulus = 21888242871839275222246405745257275088548364400416034343698204186575808495617`
//! 
//! It takes the form `-x^2 + y^2 = 1 + dx^2y^2` with
//! `d = -(168696/168700)` using the isomorphism from usual Baby Jubjub 
//! with a requirement that `a' = -1, a = 168696`, that results in 
//! ```
//! scaling = 1911982854305225074381251344103329931637610209014896889891168275855466657090 
//! a' = 21888242871839275222246405745257275088548364400416034343698204186575808495616 == -1 = a*scale^2 mod P
//! d' = 12181644023421730124874158521699555681764249180949974110617291017600649128846 == -(168696/168700) = d*scale^2
//! ```
const babyJubjubParams = {
    a: new BN("21888242871839275222246405745257275088548364400416034343698204186575808495616"),
    d: new BN("12181644023421730124874158521699555681764249180949974110617291017600649128846"),
    n: new BN("2736030358979909402780800718157159386076813972158567259200215660948447373041"),
    p: new BN("21888242871839275222246405745257275088548364400416034343698204186575808495617"),
    c: "1",
    g: ["2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7",
        "05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7"]
}

const altBabyJubjub = new elliptic.curve.edwards(babyJubjubParams);

function sign(message, privateKey, curve) {
    const r = (new BN(elliptic.rand(32), 16, "be")).umod(curve.n);
    // const r = new BN(3);
    var R = curve.g.mul(r);
    if (R.isInfinity()) {
        throw Error("R is infinity")
    }
    var s_ = (new BN(message, 16, "be")).mul(privateKey).umod(curve.n);
    var S = r.add(s_).umod(curve.n);
    return { R: R, S: S};
  };
  
function verify(message, signature, publicKey, curve) {
    var key = publicKey;
    var h = new BN(message, 16, "be");
    //console.log("C = " + h.toString(16));
    var SG = curve.g.mul(signature.S);
    var RplusAh = signature.R.add(key.mul(h));
    return RplusAh.eq(SG);
};

function serializeSignature(signature) {
    const R_X = signature.R.getX();
    const R_Y = signature.R.getY();
    const r_coords = [R_X.toString(16), R_Y.toString(16)];
    return {
        R: r_coords,
        S: signature.S.toString(16)
    };
}

function parseSignature(obj, curve) {
    const R = curve.pointFromJSON(obj.R);
    const S = new BN(obj.S, 16);
    return {R: R, S: S};
}

function floatToInteger(floatBytes, exp_bits, mantissa_bits, exp_base) {
    assert(floatBytes.length*8 == (exp_bits + mantissa_bits));
    const floatHolder = new BN(floatBytes, 16, "be"); // keep bit order
    const totalBits = floatBytes.length*8 - 1; // starts from zero
    let expBase = new BN(exp_base);
    let exponent = new BN(0);
    let exp_power_of_to = new BN(1);
    const two = new BN(2);
    for (let i = 0; i < exp_bits; i++) {
        if (floatHolder.testn(totalBits - i)) {
            exponent = exponent.add(exp_power_of_to);
        }
        exp_power_of_to = exp_power_of_to.mul(two);
    }
    exponent = expBase.pow(exponent);
    let mantissa = new BN(0);
    let mantissa_power_of_to = new BN(1);
    for (let i = 0; i < mantissa_bits; i++) {
        if (floatHolder.testn(totalBits - exp_bits - i)) {
            mantissa = mantissa.add(mantissa_power_of_to);
        }
        mantissa_power_of_to = mantissa_power_of_to.mul(two);
    }
    return exponent.mul(mantissa);
}

function integerToFloat(integer, exp_bits, mantissa_bits, exp_base, second_pass) {
    // change strategy. First try to guess the precision, and then reparse;
    const maxMantissa = (new BN(1)).ushln(mantissa_bits).subn(1);
    const maxExponent = (new BN(exp_base)).pow((new BN(1)).ushln(exp_bits).subn(1));
    assert(integer.lte(maxMantissa.mul(maxExponent)));
    // try to get the best precision
    const exponentBase = new BN(exp_base);
    let exponent = new BN(0);
    let one = new BN(1);
    if (integer.gt(maxMantissa)) {
        let exponentGuess = integer.div(maxMantissa);
        let exponentTmp = exponentGuess;

        while(exponentTmp.gte(exponentBase)) {
            exponentTmp = exponentTmp.div(exponentBase);
            exponent = exponent.addn(1);
        }
    }

    let exponentTmp = exponentBase.pow(exponent);
    if (maxMantissa.mul(exponentTmp).lt(integer)) {
        exponent = exponent.addn(1);
    }
    
    let power = exponentBase.pow(exponent);
    let mantissa = integer.div(power);
    if (!second_pass) {
        let down_to_precision = mantissa.mul(power);
        return integerToFloat(down_to_precision, exp_bits, mantissa_bits, exp_base, true);
    }
    // pack
    assert((mantissa_bits + exp_bits) % 8 === 0);
    const totalBits = mantissa_bits + exp_bits - 1;
    const encoding = new BN(0);
    for (let i = 0; i < exp_bits; i++) {
        if (exponent.testn(i)) {
            encoding.bincn(totalBits - i);
        }
    }
    for (let i = 0; i < mantissa_bits; i++) {
        if (mantissa.testn(i)) {
            encoding.bincn(totalBits - exp_bits - i);
        }
    }
    return encoding.toArrayLike(Buffer, "be", (exp_bits + mantissa_bits)/8)
}

function packBnLe(bn, numBits) {
    let bin = bn.toString(2);
    assert(bin.length <= numBits)
    bin = bin.padStart(numBits, "0");
    bin = bin.split("");
    bin = bin.reverse();
    bin = bin.join("");
    let newBN = new BN(bin, 2);
    let buff = newBN.toArrayLike(Buffer, "be");
    if (buff.length < numBits / 8) {
        buff = Buffer.concat([buff, Buffer.alloc(numBits / 8 - buff.length)])
    }
    return buff;
}

function serializeTransaction(tx) {
    const {from, to, amount, fee, nonce, good_until_block} = tx;
    assert(from.bitLength() <= 24);
    assert(to.bitLength() <= 24);
    assert(amount.bitLength() <= 128);
    assert(fee.bitLength() <= 128);
    assert(nonce.bitLength() <= 32);
    assert(good_until_block.bitLength() <= 32);

    let amountFloatBytes = integerToFloat(amount, 5, 11, 10);
    let feeFloatBytes = integerToFloat(fee, 5, 3, 10);

    const components = [
        good_until_block.toArrayLike(Buffer, "be", 4),
        nonce.toArrayLike(Buffer, "be", 4),
        packBnLe(new BN(feeFloatBytes, 16, "be"), 8),
        packBnLe(new BN(amountFloatBytes, 16, "be"), 16),
        to.toArrayLike(Buffer, "be", 3),
        from.toArrayLike(Buffer, "be", 3)
    ];

    let serialized = Buffer.concat(components);

    let newAmount = floatToInteger(amountFloatBytes, 5, 11, 10);
    //console.log("Reparsed amount = " + newAmount.toString(10));
    let newFee = floatToInteger(feeFloatBytes, 5, 3, 10);

    return {
        bytes: serialized,
        from: from,
        to: to,
        amount: newAmount,
        fee: newFee,
        nonce: nonce,
        good_until_block: good_until_block
    }
}

function getPublicData(tx) {
    const {from, to, amount, fee} = tx;
    assert(from.bitLength() <= 24);
    assert(to.bitLength() <= 24);
    assert(amount.bitLength() <= 128);
    assert(fee.bitLength() <= 128);

    const components = [];
    components.push(from.toArrayLike(Buffer, "be", 3));
    components.push(to.toArrayLike(Buffer, "be", 3));
    let amountFloatBytes = integerToFloat(amount, 5, 11, 10);
    components.push(amountFloatBytes);
    let feeFloatBytes = integerToFloat(fee, 5, 3, 10);
    components.push(feeFloatBytes);

    let serialized = Buffer.concat(components);
    let newAmount = floatToInteger(amountFloatBytes, 5, 11, 10);
    let newFee = floatToInteger(feeFloatBytes, 5, 3, 10);

    return {
        bytes: serialized,
        amount: newAmount,
        fee: newFee
    }
}

function parsePublicData(bytes) {
    assert(bytes.length % 9 === 0);
    const results = [];
    for (let i = 0; i < bytes.length/9; i++) {
        const slice = bytes.slice(9*i, 9*i + 9);
        const res = parseSlice(slice);
        results.push(res);
    }
    return results;
}

function parseSlice(slice) {
    const from = new BN(slice.slice(0, 3), 16, "be");
    const to = new BN(slice.slice(3, 6), 16, "be");
    const amount = floatToInteger(slice.slice(6, 8), 5, 11, 10)
    const fee = floatToInteger(slice.slice(8, 9), 5, 3, 10)
    return {from, to, amount, fee};
}

function toApiForm(tx, sig) {
    // expected by API server
    // pub from:               u32,
    // pub to:                 u32,
    // pub amount:             BigDecimal,
    // pub fee:                BigDecimal,
    // pub nonce:              u32,
    // pub good_until_block:   u32,
    // pub signature:          TxSignature,

    // pub struct TxSignature{
    //     pub r_x: Fr,
    //     pub r_y: Fr,
    //     pub s: Fr,
    // }

    let serializedSignature = serializeSignature(sig);
    let [r_x, r_y] = serializedSignature.R;
    let signature = {
        r_x: "0x" + r_x.padStart(64, "0"), 
        r_y: "0x" + r_y.padStart(64, "0"),
        s: "0x" + serializedSignature.S.padStart(64, "0")
    };

    let txForApi = {
        from: tx.from.toNumber(),
        to: tx.to.toNumber(),
        amount: tx.amount.toString(10),
        fee: tx.fee.toString(10),
        nonce: tx.nonce.toNumber(),
        good_until_block: tx.good_until_block.toNumber(),
        signature: signature
    }

    return txForApi;

}

function createTransaction(from, to, amount, fee, nonce, good_until_block, privateKey) {
    let tx = {
        from: new BN(from),
        to: new BN(to),
        amount: new BN(amount),
        fee: new BN(fee),
        nonce: new BN(nonce),
        good_until_block: new BN(good_until_block)
    };

    const serializedTx = serializeTransaction(tx);
    const message = serializedTx.bytes;
    //console.log("Message hex = " + message.toString("hex"));
    const signature = sign(message, privateKey, altBabyJubjub);
    const pub = altBabyJubjub.g.mul(privateKey);
    const isValid = verify(message, signature, pub, altBabyJubjub);
    assert(isValid);
    //console.log("Public = " + pub.getX().toString(16) + ", " + pub.getY().toString(16));
    const apiForm = toApiForm(serializedTx, signature);
    return apiForm;
}

function createRawTransaction(from, to, amount, fee, nonce, good_until_block, privateKey) {
    let tx = {
        from: new BN(from),
        to: new BN(to),
        amount: new BN(amount),
        fee: new BN(fee),
        nonce: new BN(nonce),
        good_until_block: new BN(good_until_block)
    };

    const serializedTx = serializeTransaction(tx);
    const message = serializedTx.bytes;
    //console.log("Message hex = " + message.toString("hex"));
    const signature = sign(message, privateKey, altBabyJubjub);
    const pub = altBabyJubjub.g.mul(privateKey);
    const isValid = verify(message, signature, pub, altBabyJubjub);
    assert(isValid);
    return {raw: message, signature: signature}
}

function test_float_parsing() {
    let bn = new BN(20474);
    let enc = integerToFloat(bn, 5, 11, 10);
    //console.log(enc.toString("hex"));
    let dec = floatToInteger(enc, 5, 11, 10);
    //console.log("dec = " + dec.toString(10));
}

function test_brute() {
    let MAX = 100000000000000;
    for (let i = 1; i < MAX; i++) {
        let bn = new BN(i);
        let enc = integerToFloat(bn, 5, 11, 10);
        let dec = floatToInteger(enc, 5, 11, 10);
        if (bn.sub(dec).gt(bn.divn(10))) {
            //console.log("Original = " + bn.toString(10) + ", decoded = " + dec.toString(10));
        }
    }
}

test_float_parsing();
// test_brute();

function test_signature_reconstruction() {
    const sk = (new BN("560fadfc60b49624b27de82891059fc9276def63e3bce85e95819bf9652c9ea", 16)).umod(altBabyJubjub.n);
    const pub_0 = altBabyJubjub.g.mul(sk);
    console.log("X = " + pub_0.getX().toString(16));
    const X = new BN("004009b16535a8058f34a642e98af661807fb8276a72c02cefeb660ffee2e41b", 16);
    const Y = new BN("1d4032b204f16d3c2a29ff5ba4836705a4c23349ff514f1c055b715e80a6bf1d", 16);
    const pub = altBabyJubjub.pointFromJSON([X, Y]);
    console.log("X = " + pub.getX().toString(16));
    let {raw, signature} = createRawTransaction(4, 3, 20474, 0, 508, 50000, sk)
    let isValid_local = verify(raw, signature, pub, altBabyJubjub);
    assert(isValid_local);
    const parsedSig = parseSignature({
        R: ["15157418731234bed372c84b4de28c994028f8290689e3a21571ecde49644505",
            "0d081beb4eaf31a9ba4b86fbfda6225fd65128a1decf53be44d9cab457eec0cd"
        ],
        S: "023012a357fe05a3bbd4f0b360dd60436cc54d333173e5bf7dfafd13192df47e"
    }, altBabyJubjub);
    let isValid = verify(raw, parsedSig, pub, altBabyJubjub);
    assert(isValid);
}

// test_signature_reconstruction();

function main() {
    const sk = (new BN(elliptic.rand(32), 16, "be")).umod(altBabyJubjub.n);
    const pub = altBabyJubjub.g.mul(sk);
    // const tx = createTransaction(3, 4, 20474, 0, 1, 100, sk);

    // const message = serializeTransaction(tx.from, tx.to, tx.amount, tx.nonce, tx.good_until_block);
    const message = elliptic.rand(16);
    const signature = sign(message, sk, altBabyJubjub);
    const isValid = verify(message, signature, pub, altBabyJubjub);
    assert(isValid);
    const sigJSON = serializeSignature(signature);
    const parsedSig = parseSignature(sigJSON, altBabyJubjub);
    const isValidParsed = verify(message, parsedSig, pub, altBabyJubjub);
    assert(isValidParsed);
}

function newKey(seed) {
    const sk = (new BN(seed || elliptic.rand(32), 16, "be")).umod(altBabyJubjub.n);
    const pub = altBabyJubjub.g.mul(sk);
    let y = pub.getY();
    const x = pub.getX();
    const x_parity = x.testn(0);
    let y_packed = y;
    if (x_parity) {
        y_packed = y_packed.add(((new BN(1)).ushln(255)));
    }
    return {
        privateKey: sk,
        publicKey: {x: x, y: y},
        packedPublicKey: y_packed
    };
}

// main();

export default {
    sign,
    verify,
    floatToInteger,
    integerToFloat,
    serializeSignature,
    serializeTransaction,
    parseSignature,
    newKey,
    getPublicData,
    parsePublicData,
    createTransaction
}
