import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, deployGovernance} from "./deploy";

import {expect, use, assert} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther, hexlify} from "ethers/utils";
import {createDepositPublicData, createWithdrawPublicData, createFullExitPublicData, hex_to_ascii} from "./helpers"

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "0809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("FAILS", function() {
    this.timeout(50000);

    let franklinDeployedContract;
    let governanceDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        governanceDeployedContract = await deployGovernance(wallet, wallet.address);
        franklinDeployedContract = await deployFranklin(wallet, governanceDeployedContract.address);
        // erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
        // // Make sure that exit wallet can execute transactions.
        // await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("ETH: wrong deposit value", async () => {
        // ETH: Wrong deposit value
        // const depositValue = parseEther("0.005"); // the value passed to tx
        // let tx = await franklinDeployedContract.depositETH(
        //     franklinAddressBinary,
        //     {
        //         value: depositValue,
        //         gasLimit: bigNumberify("500000")
        //     }
        // );

        // await tx.wait()
        // .then(() => {
        //     throw("This should not be ok");
        // })
        // .catch();

        // const code = await provider.call(tx, tx.blockNumber);
        // const reason = hex_to_ascii(code.substr(138));
        
        // expect(reason.substring(0,5)).equal("fdh11");
    });

    // it("ETH: wrong deposit value", async () => {
    //     // ETH: Wrong deposit value
    //     const depositValue = parseEther("0.005"); // the value passed to tx
    //     let tx = await franklinDeployedContract.depositETH(
    //         franklinAddressBinary,
    //         {
    //             value: depositValue,
    //             gasLimit: bigNumberify("500000")
    //         }
    //     );

    //     await tx.wait()
    //     .then(() => {
    //         throw("This should not be ok");
    //     })
    //     .catch();

    //     const code = await provider.call(tx, tx.blockNumber);
    //     const reason = hex_to_ascii(code.substr(138));
        
    //     expect(reason.substring(0,5)).equal("fdh11");
    // });
});
