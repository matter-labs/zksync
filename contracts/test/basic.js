const chai = require('chai');
const {
    createMockProvider,
    deployContract,
    getWallets,
    solidity
} = require('ethereum-waffle');
const FranklinContract = require('../build/Franklin');
const ERC20Contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
const ethers = require('ethers');


chai.use(solidity);
const {
    expect
} = chai;

// describe('INTEGRATION: Example', () => {
//     let provider = createMockProvider();
//     let [wallet, walletTo] = getWallets(provider);

//     let franklinContract;
//     let erc20_1;

//     let waitBlocks = async function(amount) {
//         for (let i = 0; i < amount; i++) {
//             await provider.send("evm_mine",[]);
//         }
//     };

//     beforeEach(async () => {
//         // To make contracts built with truffle compatible with waffle.
//         ERC20Contract.evm = {
//             bytecode: ERC20Contract.bytecode
//         };

//         franklinContract = await deployContract(wallet, FranklinContract, [ethers.constants.HashZero, wallet.address, wallet.address], {
//             gasLimit: 6000000
//         });
//         erc20_1 = await deployContract(wallet, ERC20Contract, []);
//         await franklinContract.addToken(erc20_1.address);
//     });

//     it('ERC20 deposit, withdraw', async () => {
//         await erc20_1.mint(wallet.address, 100);
//         await erc20_1.approve(franklinContract.address, 25);

//         // Make deposit
//         await franklinContract.depositERC20(erc20_1.address, 25);
//         expect(await erc20_1.balanceOf(wallet.address)).to.eq(75);
//         expect(await erc20_1.balanceOf(franklinContract.address)).to.eq(25);

//         // Locked withdraw attempt
//         await expect(franklinContract.withdrawERC20(erc20_1.address, 25)).to.be.revertedWith("balance locked");

//         // Wait and withdraw again
//         await waitBlocks(8 * 60);
//         await franklinContract.withdrawERC20(erc20_1.address, 25);
//         expect(await erc20_1.balanceOf(wallet.address)).to.eq(100);
//         expect(await erc20_1.balanceOf(franklinContract.address)).to.eq(0);
//     }).timeout(10000);

// });