import BN = require("bn.js");
import { curve } from "elliptic";
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { sha256 } from "js-sha256";
import edwards = curve.edwards;
import { Signature } from "./types";

const blake2b = require("blake2b");
const elliptic = require("elliptic");
const crypto = require("crypto");

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
const babyJubjubParams = {
  a: new BN(
    "21888242871839275222246405745257275088548364400416034343698204186575808495616"
  ),
  d: new BN(
    "12181644023421730124874158521699555681764249180949974110617291017600649128846"
  ),
  n: new BN(
    "2736030358979909402780800718157159386076813972158567259200215660948447373041"
  ),
  p: new BN(
    "21888242871839275222246405745257275088548364400416034343698204186575808495617"
  ),
  c: "1",
  g: [
    "2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7",
    "05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7"
  ]
};

const fsModulus = babyJubjubParams.n;
const fsOne = new BN(1); // (new BN(2)).pow(new BN(256)).mod(fsModulus);
const fsZero = new BN(0);
export const altjubjubCurve = new elliptic.curve.edwards(babyJubjubParams);
const curveZero = altjubjubCurve.point("0", "1");
const chunksPerGenerator = 62;
export const addressLen = 20;
const PAD_MSG_BEFORE_HASH_BYTES_LEN = 92;

const gen1 = altjubjubCurve.point(
  "184570ed4909a81b2793320a26e8f956be129e4eed381acf901718dff8802135",
  "1c3a9a830f61587101ef8cbbebf55063c1c6480e7e5a7441eac7f626d8f69a45"
);
const gen2 = altjubjubCurve.point(
  "0afc00ffa0065f5479f53575e86f6dcd0d88d7331eefd39df037eea2d6f031e4",
  "237a6734dd50e044b4f44027ee9e70fcd2e5724ded1d1c12b820a11afdc15c7a"
);
const gen3 = altjubjubCurve.point(
  "00fb62ad05ee0e615f935c5a83a870f389a5ea2baccf22ad731a4929e7a75b37",
  "00bc8b1c9d376ceeea2cf66a91b7e2ad20ab8cce38575ac13dbefe2be548f702"
);
const gen4 = altjubjubCurve.point(
  "0675544aa0a708b0c584833fdedda8d89be14c516e0a7ef3042f378cb01f6e48",
  "169025a530508ee4f1d34b73b4d32e008b97da2147f15af3c53f405cf44f89d4"
);
const gen5 = altjubjubCurve.point(
  "07350a0660a05014168047155c0a0647ea2720ecb182a6cb137b29f8a5cfd37f",
  "3004ad73b7abe27f17ec04b04b450955a4189dd012b4cf4b174af15bd412696a"
);
const basicGenerators = [gen1, gen2, gen3, gen4, gen5];

function wrapScalar(fs: BN): BN {
  while (fs.ltn(0)) {
    fs = fs.add(fsModulus);
  }
  if (fs.gte(fsModulus)) {
    fs = fs.mod(fsModulus);
  }
  return fs;
}

let generatorExpTable;

function lookupGeneratorFromTable(generator, window, idx) {
  if (!generatorExpTable) {
    generatorExpTable = genPedersonHashLookupTable();
  }
  return generatorExpTable[generator][window][idx];
}

function calulateGenerator(generator, window, idx) {
  const basePower = new BN(256).pow(new BN(window));
  const power = basePower.muln(idx);
  return basicGenerators[generator].mul(power).normalize();
}

function genPedersonHashLookupTable() {
  function genTableForGenerator(g) {
    const result = [];
    for (let window = 0; window < 32; ++window) {
      const window_table = [curveZero];
      let accum = curveZero;
      for (let mul = 1; mul < 256; ++mul) {
        accum = accum.add(g).normalize();
        window_table.push(accum);
      }
      g = g.mul(new BN(256));
      result.push(window_table);
    }
    return result;
  }

  const table = [];
  for (const g of basicGenerators) {
    table.push(genTableForGenerator(g));
  }
  return table;
}

function buffer2bits_le(buff) {
  const res = new Array(buff.length * 8);
  for (let i = 0; i < buff.length; i++) {
    const b = buff[i];
    res[i * 8] = (b & 0x01) != 0;
    res[i * 8 + 1] = (b & 0x02) != 0;
    res[i * 8 + 2] = (b & 0x04) != 0;
    res[i * 8 + 3] = (b & 0x08) != 0;
    res[i * 8 + 4] = (b & 0x10) != 0;
    res[i * 8 + 5] = (b & 0x20) != 0;
    res[i * 8 + 6] = (b & 0x40) != 0;
    res[i * 8 + 7] = (b & 0x80) != 0;
  }
  return res;
}

function buffer2bits_be(buff) {
  const res = new Array(buff.length * 8);
  for (let i = 0; i < buff.length; i++) {
    const b = buff[i];
    res[i * 8] = (b & 0x80) != 0;
    res[i * 8 + 1] = (b & 0x40) != 0;
    res[i * 8 + 2] = (b & 0x20) != 0;
    res[i * 8 + 3] = (b & 0x10) != 0;
    res[i * 8 + 4] = (b & 0x08) != 0;
    res[i * 8 + 5] = (b & 0x04) != 0;
    res[i * 8 + 6] = (b & 0x02) != 0;
    res[i * 8 + 7] = (b & 0x01) != 0;
  }
  return res;
}

export function pedersenHash(
  input: Buffer,
  bit_endianness: "le" | "be" = "le"
): EdwardsPoint {
  const personaizationBits = new Array(6).fill(true);
  let bits;
  if (bit_endianness == "le") {
    bits = personaizationBits.concat(buffer2bits_le(input));
  } else {
    bits = personaizationBits.concat(buffer2bits_be(input));
  }

  function fsToPoint(fs, generator) {
    fs = wrapScalar(fs);

    let tmpPoint = curveZero;
    const accStr = fs.toString("hex").padStart(64, "0");
    const accBuff = Buffer.from(accStr, "hex").reverse();
    for (let window = 0; window < 32; ++window) {
      tmpPoint = tmpPoint.add(
        calulateGenerator(generator, window, accBuff[window])
      );
    }
    return tmpPoint;
  }

  while (bits.length % 3 != 0) {
    bits.push(false);
  }

  let result = curveZero;

  let newChunkEncountered = false;
  let currentTriple = 0;
  let currentGenerator = 0;

  let generatorChunksLeft = chunksPerGenerator;

  let acc = fsZero;
  let cur = fsOne;

  while (bits.length > 0) {
    const triple = bits.slice(0, 3);
    bits = bits.slice(3);
    ++currentTriple;
    generatorChunksLeft -= 1;
    newChunkEncountered = true;

    let tmp = cur;
    const [a, b, c] = triple;
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

function to_uniform(bytes: Buffer): BN {
  const bits = new Array(bytes.length * 8);
  let bit_n = 0;
  for (let i = bytes.length - 1; i >= 0; --i) {
    const b = bytes[i];
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

  let res = new BN(0);
  for (let n = 0; n < bits.length; n++) {
    res = res.muln(2);
    if (bits[n]) {
      res = res.addn(1);
    }
  }

  return wrapScalar(res);
}

function balke2bHStar(a: Buffer, b: Buffer): BN {
  let output = new Uint8Array(64);
  const hash = blake2b(64, null, null, Buffer.from("Zcash_RedJubjubH"));
  hash.update(a);
  hash.update(b);
  output = hash.digest();
  const buff = Buffer.from(output);

  return to_uniform(buff);
}

function sha256HStart(a: Buffer, b: Buffer): BN {
  const hasher = sha256.create();
  const personaization = "";
  hasher.update(personaization);
  hasher.update(a);
  hasher.update(b);
  const hash = Buffer.from(hasher.array());
  const point = to_uniform(hash);
  // console.log("sha256: ", hash.toString("hex"));
  // console.log("sha256 point: ", point);
  return point;
}

function pedersenHStar(input: Buffer): BN {
  const p_hash_start_res = pedersenHash(input);
  const p_hash_star_fe = to_uniform(
    p_hash_start_res.getX().toArrayLike(Buffer, "le", 32)
  );
  return p_hash_star_fe;
}

export function musigSHA256(priv_key: BN, msg: Buffer): Signature {
  const msgToHash = Buffer.alloc(PAD_MSG_BEFORE_HASH_BYTES_LEN, 0);
  msg.copy(msgToHash);
  msg = pedersenHash(msgToHash, "be")
    .getX()
    .toArrayLike(Buffer, "le", 32);

  const t = crypto.randomBytes(80);

  const pub_key = privateKeyToPublicKey(priv_key);
  const pk_bytes = pub_key.getX().toArrayLike(Buffer, "le", 32);

  const r = balke2bHStar(t, msg);
  const r_g = altjubjubCurve.g.mul(r);
  const r_g_bytes = r_g.getX().toArrayLike(Buffer, "le", 32);

  const concat = Buffer.concat([pk_bytes, r_g_bytes]);

  const msg_padded = Buffer.alloc(32, 0);
  msg.copy(msg_padded, 0, 0, 32);

  const s = wrapScalar(
    sha256HStart(concat, msg_padded)
      .mul(priv_key)
      .add(r)
  );

  const signature = Buffer.concat([
    serializePointPacked(r_g),
    s.toArrayLike(Buffer, "le", 32)
  ]).toString("hex");
  const publicKey = serializePointPacked(pub_key).toString("hex");
  return { publicKey, signature };
}

export function musigPedersen(priv_key: BN, msg: Buffer): Signature {
  const msgToHash = Buffer.alloc(PAD_MSG_BEFORE_HASH_BYTES_LEN, 0);
  msg.copy(msgToHash);
  msg = pedersenHash(msgToHash, "be")
    .getX()
    .toArrayLike(Buffer, "le", 32);

  const t = crypto.randomBytes(80);

  const pub_key = privateKeyToPublicKey(priv_key);
  const pk_bytes = pub_key.getX().toArrayLike(Buffer, "le", 32);

  const r = balke2bHStar(t, msg);
  const r_g = altjubjubCurve.g.mul(r);
  const r_g_bytes = r_g.getX().toArrayLike(Buffer, "le", 32);

  const concat = Buffer.concat([pk_bytes, r_g_bytes]);
  const concat_hash_bytes = pedersenHash(concat)
    .getX()
    .toArrayLike(Buffer, "le", 32);

  const msg_padded = Buffer.alloc(32, 0);
  msg.copy(msg_padded, 0, 0, 32);

  const s = wrapScalar(
    pedersenHStar(Buffer.concat([concat_hash_bytes, msg_padded]))
      .mul(priv_key)
      .add(r)
  );

  const signature = Buffer.concat([
    serializePointPacked(r_g),
    s.toArrayLike(Buffer, "le", 32)
  ]).toString("hex");
  const publicKey = serializePointPacked(pub_key).toString("hex");
  return { publicKey, signature };
}

export function privateKeyToPublicKey(pk: BN): edwards.EdwardsPoint {
  return altjubjubCurve.g.mul(pk);
}

export function pubkeyToAddress(pubKey: edwards.EdwardsPoint): Buffer {
  const x = pubKey.getX().toArrayLike(Buffer, "le", 32);
  const y = pubKey.getY().toArrayLike(Buffer, "le", 32);
  const res = pedersenHash(Buffer.concat([x, y]))
    .getX()
    .toArrayLike(Buffer, "le", 32)
    .slice(0, addressLen)
    .reverse();
  return res;
}

export function serializePointPacked(point: edwards.EdwardsPoint): Buffer {
  const y = point.getY();
  const y_buff = y.toArrayLike(Buffer, "le", 32);

  if (
    altjubjubCurve
      .pointFromY(y, true)
      .getX()
      .eq(point.getX())
  ) {
    // x is odd
    y_buff[y_buff.length - 1] |= 1 << 7;
  }
  return y_buff;
}

export function signTransactionBytes(privKey: BN, bytes: Buffer): Signature {
  return musigPedersen(privKey, bytes);
}

export function privateKeyFromSeed(seed: Buffer): BN {
  if (seed.length < 32) {
    throw new Error("Seed is too short");
  }
  let effectiveSeed = new Uint8Array(seed);
  while (true) {
    const hasher = sha256.create();
    hasher.update(effectiveSeed);
    const hashResult = new Uint8Array(hasher.arrayBuffer());
    const privateKey = new BN(hashResult);
    if (privateKey.gte(fsModulus)) {
      effectiveSeed = hashResult;
      continue;
    }
    return privateKey;
  }
}
