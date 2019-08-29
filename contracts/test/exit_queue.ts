import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, deployExitQueue} from "../src.ts/deploy";

import {BN} from "bn.js";
import {expect, use} from "chai";
import {solidity} from "ethereum-waffle";
import {BigNumber, bigNumberify, parseEther} from "ethers/utils";
import {packAmount, packFee} from "../../js/franklin_lib/src/utils";

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "010203040506070809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

