"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var BN = require("bn.js");
var js_sha256_1 = require("js-sha256");
var utils_1 = require("./utils");
var blake2b = require("blake2b");
var elliptic = require("elliptic");
var crypto = require("crypto");
// ! `Fr modulus = 21888242871839275222246405745257275088548364400416034343698204186575808495617`
// !
// ! It takes the form `-x^2 + y^2 = 1 + dx^2y^2` with
// ! `d = -(168696/168700)` using the isomorphism from usual Baby Jubjub
// ! with a requirement that `a' = -1, a = 168696`, that results in
// ! ```
// ! scaling = 1911982854305225074381251344103329931637610209014896889891168275855466657090
// ! a' = 21888242871839275222246405745257275088548364400416034343698204186575808495616 == -1 = a*scale^2 mod P
// ! d' = 12181644023421730124874158521699555681764249180949974110617291017600649128846 == -(168696/168700) = d*scale^2
// ! ```
var babyJubjubParams = {
    a: new BN("21888242871839275222246405745257275088548364400416034343698204186575808495616"),
    d: new BN("12181644023421730124874158521699555681764249180949974110617291017600649128846"),
    n: new BN("2736030358979909402780800718157159386076813972158567259200215660948447373041"),
    p: new BN("21888242871839275222246405745257275088548364400416034343698204186575808495617"),
    c: "1",
    g: [
        "2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7",
        "05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7"
    ]
};
var fsModulus = babyJubjubParams.n;
var fsOne = new BN(1);
var fsZero = new BN(0);
exports.altjubjubCurve = new elliptic.curve.edwards(babyJubjubParams);
var curveZero = exports.altjubjubCurve.point("0", "1");
var chunksPerGenerator = 62;
exports.addressLen = 20;
var PAD_MSG_BEFORE_HASH_BYTES_LEN = 92;
var gen1 = exports.altjubjubCurve.point("184570ed4909a81b2793320a26e8f956be129e4eed381acf901718dff8802135", "1c3a9a830f61587101ef8cbbebf55063c1c6480e7e5a7441eac7f626d8f69a45");
var gen2 = exports.altjubjubCurve.point("0afc00ffa0065f5479f53575e86f6dcd0d88d7331eefd39df037eea2d6f031e4", "237a6734dd50e044b4f44027ee9e70fcd2e5724ded1d1c12b820a11afdc15c7a");
var gen3 = exports.altjubjubCurve.point("00fb62ad05ee0e615f935c5a83a870f389a5ea2baccf22ad731a4929e7a75b37", "00bc8b1c9d376ceeea2cf66a91b7e2ad20ab8cce38575ac13dbefe2be548f702");
var gen4 = exports.altjubjubCurve.point("0675544aa0a708b0c584833fdedda8d89be14c516e0a7ef3042f378cb01f6e48", "169025a530508ee4f1d34b73b4d32e008b97da2147f15af3c53f405cf44f89d4");
var gen5 = exports.altjubjubCurve.point("07350a0660a05014168047155c0a0647ea2720ecb182a6cb137b29f8a5cfd37f", "3004ad73b7abe27f17ec04b04b450955a4189dd012b4cf4b174af15bd412696a");
var basicGenerators = [gen1, gen2, gen3, gen4, gen5];
function wrapScalar(fs) {
    while (fs.ltn(0)) {
        fs = fs.add(fsModulus);
    }
    if (fs.gte(fsModulus)) {
        fs = fs.mod(fsModulus);
    }
    return fs;
}
var generatorExpTable;
function lookupGeneratorFromTable(generator, window, idx) {
    if (!generatorExpTable) {
        generatorExpTable = genPedersonHashLookupTable();
    }
    return generatorExpTable[generator][window][idx];
}
function calulateGenerator(generator, window, idx) {
    var basePower = new BN(256).pow(new BN(window));
    var power = basePower.muln(idx);
    return basicGenerators[generator].mul(power).normalize();
}
function genPedersonHashLookupTable() {
    function genTableForGenerator(g) {
        var result = [];
        for (var window_1 = 0; window_1 < 32; ++window_1) {
            var window_table = [curveZero];
            var accum = curveZero;
            for (var mul = 1; mul < 256; ++mul) {
                accum = accum.add(g).normalize();
                window_table.push(accum);
            }
            g = g.mul(new BN(256));
            result.push(window_table);
        }
        return result;
    }
    var table = [];
    for (var _i = 0, basicGenerators_1 = basicGenerators; _i < basicGenerators_1.length; _i++) {
        var g = basicGenerators_1[_i];
        table.push(genTableForGenerator(g));
    }
    return table;
}
function pedersenHash(input, bit_endianness) {
    if (bit_endianness === void 0) { bit_endianness = "le"; }
    var personaizationBits = new Array(6).fill(true);
    var bits;
    if (bit_endianness == "le") {
        bits = personaizationBits.concat(utils_1.buffer2bitsLE(input));
    }
    else {
        bits = personaizationBits.concat(utils_1.buffer2bitsBE(input));
    }
    function fsToPoint(fs, generator) {
        fs = wrapScalar(fs);
        var tmpPoint = curveZero;
        var accStr = fs.toString("hex").padStart(64, "0");
        var accBuff = Buffer.from(accStr, "hex").reverse();
        for (var window_2 = 0; window_2 < 32; ++window_2) {
            tmpPoint = tmpPoint.add(calulateGenerator(generator, window_2, accBuff[window_2]));
        }
        return tmpPoint;
    }
    while (bits.length % 3 != 0) {
        bits.push(false);
    }
    var result = curveZero;
    var newChunkEncountered = false;
    var currentTriple = 0;
    var currentGenerator = 0;
    var generatorChunksLeft = chunksPerGenerator;
    var acc = fsZero;
    var cur = fsOne;
    while (bits.length > 0) {
        var triple = bits.slice(0, 3);
        bits = bits.slice(3);
        ++currentTriple;
        generatorChunksLeft -= 1;
        newChunkEncountered = true;
        var tmp = cur;
        var a = triple[0], b = triple[1], c = triple[2];
        if (a) {
            tmp = tmp.add(cur);
        }
        cur = cur.muln(2);
        if (b) {
            tmp = tmp.add(cur);
        }
        if (c) {
            tmp = tmp.neg();
        }
        acc = acc.add(tmp);
        cur = cur.muln(8);
        if (generatorChunksLeft == 0) {
            result = result.add(fsToPoint(acc, currentGenerator));
            ++currentGenerator;
            generatorChunksLeft = chunksPerGenerator;
            acc = fsZero;
            cur = fsOne;
            newChunkEncountered = false;
        }
    }
    if (newChunkEncountered) {
        result = result.add(fsToPoint(acc, currentGenerator));
    }
    return result.normalize();
}
exports.pedersenHash = pedersenHash;
function to_uniform(bytes) {
    var bits = new Array(bytes.length * 8);
    var bit_n = 0;
    for (var i = bytes.length - 1; i >= 0; --i) {
        var b = bytes[i];
        bits[bit_n] = (b & 0x80) != 0;
        bits[bit_n + 1] = (b & 0x40) != 0;
        bits[bit_n + 2] = (b & 0x20) != 0;
        bits[bit_n + 3] = (b & 0x10) != 0;
        bits[bit_n + 4] = (b & 0x08) != 0;
        bits[bit_n + 5] = (b & 0x04) != 0;
        bits[bit_n + 6] = (b & 0x02) != 0;
        bits[bit_n + 7] = (b & 0x01) != 0;
        bit_n += 8;
    }
    var res = new BN(0);
    for (var n = 0; n < bits.length; n++) {
        res = res.muln(2);
        if (bits[n]) {
            res = res.addn(1);
        }
    }
    return wrapScalar(res);
}
function balke2bHStar(a, b) {
    var output = new Uint8Array(64);
    var hash = blake2b(64, null, null, Buffer.from("Zcash_RedJubjubH"));
    hash.update(a);
    hash.update(b);
    output = hash.digest();
    var buff = Buffer.from(output);
    return to_uniform(buff);
}
function sha256HStart(a, b) {
    var hasher = js_sha256_1.sha256.create();
    var personaization = "";
    hasher.update(personaization);
    hasher.update(a);
    hasher.update(b);
    var hash = Buffer.from(hasher.array());
    return to_uniform(hash);
}
function pedersenHStar(input) {
    var p_hash_start_res = pedersenHash(input);
    var p_hash_star_fe = to_uniform(p_hash_start_res.getX().toArrayLike(Buffer, "le", 32));
    return p_hash_star_fe;
}
function musigSHA256(priv_key, msg) {
    var msgToHash = Buffer.alloc(PAD_MSG_BEFORE_HASH_BYTES_LEN, 0);
    msg.copy(msgToHash);
    msg = pedersenHash(msgToHash, "be")
        .getX()
        .toArrayLike(Buffer, "le", 32);
    var t = crypto.randomBytes(80);
    var pub_key = privateKeyToPublicKey(priv_key);
    var pk_bytes = pub_key.getX().toArrayLike(Buffer, "le", 32);
    var r = balke2bHStar(t, msg);
    var r_g = exports.altjubjubCurve.g.mul(r);
    var r_g_bytes = r_g.getX().toArrayLike(Buffer, "le", 32);
    var concat = Buffer.concat([pk_bytes, r_g_bytes]);
    var msg_padded = Buffer.alloc(32, 0);
    msg.copy(msg_padded, 0, 0, 32);
    var s = wrapScalar(sha256HStart(concat, msg_padded)
        .mul(priv_key)
        .add(r));
    var signature = Buffer.concat([
        serializePointPacked(r_g),
        s.toArrayLike(Buffer, "le", 32)
    ]).toString("hex");
    var publicKey = serializePointPacked(pub_key).toString("hex");
    return { pubKey: publicKey, signature: signature };
}
exports.musigSHA256 = musigSHA256;
function musigPedersen(priv_key, msg) {
    var msgToHash = Buffer.alloc(PAD_MSG_BEFORE_HASH_BYTES_LEN, 0);
    msg.copy(msgToHash);
    msg = pedersenHash(msgToHash, "be")
        .getX()
        .toArrayLike(Buffer, "le", 32);
    var t = crypto.randomBytes(80);
    var pub_key = privateKeyToPublicKey(priv_key);
    var pk_bytes = pub_key.getX().toArrayLike(Buffer, "le", 32);
    var r = balke2bHStar(t, msg);
    var r_g = exports.altjubjubCurve.g.mul(r);
    var r_g_bytes = r_g.getX().toArrayLike(Buffer, "le", 32);
    var concat = Buffer.concat([pk_bytes, r_g_bytes]);
    var concat_hash_bytes = pedersenHash(concat)
        .getX()
        .toArrayLike(Buffer, "le", 32);
    var msg_padded = Buffer.alloc(32, 0);
    msg.copy(msg_padded, 0, 0, 32);
    var s = wrapScalar(pedersenHStar(Buffer.concat([concat_hash_bytes, msg_padded]))
        .mul(priv_key)
        .add(r));
    var signature = Buffer.concat([
        serializePointPacked(r_g),
        s.toArrayLike(Buffer, "le", 32)
    ]).toString("hex");
    var publicKey = serializePointPacked(pub_key).toString("hex");
    return { pubKey: publicKey, signature: signature };
}
exports.musigPedersen = musigPedersen;
function privateKeyToPublicKey(pk) {
    return exports.altjubjubCurve.g.mul(pk);
}
exports.privateKeyToPublicKey = privateKeyToPublicKey;
function pubkeyToAddress(pubKey) {
    var x = pubKey.getX().toArrayLike(Buffer, "le", 32);
    var y = pubKey.getY().toArrayLike(Buffer, "le", 32);
    var res = pedersenHash(Buffer.concat([x, y]))
        .getX()
        .toArrayLike(Buffer, "le", 32)
        .slice(0, exports.addressLen)
        .reverse();
    return res;
}
exports.pubkeyToAddress = pubkeyToAddress;
function serializePointPacked(point) {
    var y = point.getY();
    var y_buff = y.toArrayLike(Buffer, "le", 32);
    if (exports.altjubjubCurve
        .pointFromY(y, true)
        .getX()
        .eq(point.getX())) {
        // x is odd
        y_buff[y_buff.length - 1] |= 1 << 7;
    }
    return y_buff;
}
exports.serializePointPacked = serializePointPacked;
function signTransactionBytes(privKey, bytes) {
    return musigPedersen(privKey, bytes);
}
exports.signTransactionBytes = signTransactionBytes;
function privateKeyFromSeed(seed) {
    if (seed.length < 32) {
        throw new Error("Seed is too short");
    }
    var effectiveSeed = new Uint8Array(seed);
    while (true) {
        var hasher = js_sha256_1.sha256.create();
        hasher.update(effectiveSeed);
        var hashResult = new Uint8Array(hasher.arrayBuffer());
        var privateKey = new BN(hashResult);
        if (privateKey.gte(fsModulus)) {
            effectiveSeed = hashResult;
            continue;
        }
        return privateKey;
    }
}
exports.privateKeyFromSeed = privateKeyFromSeed;
