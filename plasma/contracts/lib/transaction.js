const elliptic = require('elliptic');
const BN = require("bn.js");
const assert = require("assert");
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

function sign(message, priveteKey, curve) {
    const r = (new BN(elliptic.rand(32), 16, "be")).umod(curve.n);
    var R = curve.g.mul(r);
    if (R.isInfinity()) {
        throw Error("R is infinity")
    }
    var s_ = (new BN(message, 16, "be")).mul(priveteKey);
    var S = r.add(s_).umod(curve.n);
    return { R: R, S: S};
  };
  
function verify(message, signature, publicKey, curve) {
    var key = publicKey;
    var h = new BN(message, 16, "be");
    var SG = curve.g.mul(signature.S);
    var RplusAh = signature.R.add(key.mul(h));
    return RplusAh.eq(SG);
};

function serializeSignature(signature) {
    const R_X = signature.R.getX();
    const R_Y = signature.R.getY();
    return {
        R: [R_X.toString(16), R_Y.toString(16)],
        S: signature.S.toString(16)
    };
}

function parseSignature(obj, curve) {
    const R = curve.pointFromJSON(obj.R);
    const S = new BN(obj.S, 16);
    return {R: R, S: S};
}

function main() {
    const sk = (new BN(elliptic.rand(32), 16, "be")).umod(altBabyJubjub.n);
    const pub = altBabyJubjub.g.mul(sk);
    const message = elliptic.rand(16);
    const signature = sign(message, sk, altBabyJubjub);
    const isValid = verify(message, signature, pub, altBabyJubjub);
    assert(isValid);
    const sigJSON = serializeSignature(signature);
    const parsedSig = parseSignature(sigJSON, altBabyJubjub);
    const isValidParsed = verify(message, parsedSig, pub, altBabyJubjub);
    assert(isValidParsed);
}

main();