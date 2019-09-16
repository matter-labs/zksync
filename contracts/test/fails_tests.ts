// import {ethers} from "ethers";
// import {addTestERC20Token, deployFranklin, deployGovernance} from "./deploy";

// import {expect, use, assert} from "chai";
// import {solidity} from "ethereum-waffle";
// import {bigNumberify, parseEther, hexlify, BigNumber} from "ethers/utils";
// import {createDepositPublicData, createWithdrawPublicData, createFullExitPublicData, hex_to_ascii} from "./helpers"

// use(solidity);

// const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
// const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
// const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
// const franklinAddress = "0809101112131415161718192021222334252627";
// const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
// const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

// describe("FAILS", function() {
//     this.timeout(50000);

//     let franklinDeployedContract;
//     let governanceDeployedContract;
//     let erc20DeployedToken;

//     beforeEach(async () => {
//         governanceDeployedContract = await deployGovernance(wallet, wallet.address);
//         franklinDeployedContract = await deployFranklin(wallet, governanceDeployedContract.address);
//         erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
//         // Make sure that exit wallet can execute transactions.
//         // await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
//     });

//     it("Deposit", async () => {
//         // ETH: Wrong tx value (msg.value >= fee)
//         // const depositETH1Value = parseEther("0.005"); // the value passed to tx
//         // let tx1 = await franklinDeployedContract.depositETH(
//         //     franklinAddressBinary,
//         //     {
//         //         value: depositETH1Value,
//         //         gasLimit: bigNumberify("500000")
//         //     }
//         // );

//         // await tx1.wait()
//         // .catch(() => {});

//         // const code1 = await provider.call(tx1, tx1.blockNumber);
//         // const reason1 = hex_to_ascii(code1.substr(138));
        
//         // expect(reason1.substring(0,5)).equal("fdh11");

//         // ETH: Wrong tx value (amount <= MAX_VALUE)
//         const depositETH2Value = parseEther("999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999.3"); // the value passed to tx
//         let tx2 = await franklinDeployedContract.depositETH(
//             franklinAddressBinary,
//             {
//                 value: depositETH2Value,
//                 gasLimit: bigNumberify("500000")
//             }
//         );

//         await tx2.wait()
//         .catch(() => {});

//         const code2 = await provider.call(tx2, tx2.blockNumber);
//         const reason2 = hex_to_ascii(code2.substr(138));
        
//         expect(reason2.substring(0,5)).equal("fdh12");

//         // // ERC20: Wrong tx value (msg.value >= fee)
//         // let erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);

//         // const depositERCValue = 78;
//         // const feeValue = parseEther("0.001");
//         // await erc20DeployedToken.approve(franklinDeployedContract.address, depositERCValue);

//         // let tx2 = await franklinDeployedContract.depositERC20(
//         //     erc20DeployedToken.address,
//         //     depositERCValue, 
//         //     franklinAddressBinary,
//         //     {value: feeValue, gasLimit: bigNumberify("500000")}
//         // );

//         // await tx2.wait()
//         // .then(() => {
//         //     throw("tx2 is ok");
//         // })
//         // .catch(() => {});

//         // const code2 = await provider.call(tx2, tx2.blockNumber);
//         // const reason2 = hex_to_ascii(code2.substr(138));
        
//         // expect(reason2.substring(0,5)).equal("fd011");
//     });

//     // it("ETH: wrong deposit value", async () => {
//     //     // ETH: Wrong deposit value
//     //     const depositValue = parseEther("0.005"); // the value passed to tx
//     //     let tx = await franklinDeployedContract.depositETH(
//     //         franklinAddressBinary,
//     //         {
//     //             value: depositValue,
//     //             gasLimit: bigNumberify("500000")
//     //         }
//     //     );

//     //     await tx.wait()
//     //     .then(() => {
//     //         throw("This should not be ok");
//     //     })
//     //     .catch();

//     //     const code = await provider.call(tx, tx.blockNumber);
//     //     const reason = hex_to_ascii(code.substr(138));
        
//     //     expect(reason.substring(0,5)).equal("fdh11");
//     // });
// });
