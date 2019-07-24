const deployLib = require('../scripts/deploy.js');
const etherLib = require('../scripts/ether.js');
const erc20Lib = require('../scripts/erc20_token.js');
const assert = require("assert");
const ethUtils = require("ethereumjs-util");
const BN = require("bn.js");
const ethers = require('ethers');
const chai = require('chai');
const {createMockProvider, deployContract, getWallets, solidity} = require('ethereum-waffle');


chai.use(solidity);
const {expect} = chai;

const franklinContract = require('../build/Franklin');
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

describe('INTEGRATION: Deposit', () => {
    let franklinDeployedContract;
    let erc20DeployedToken;
  
    beforeEach(async () => {
        franklinDeployedContract = await deployLib.deployFranklin(wallet, franklinContract);
        erc20DeployedToken = await erc20Lib.deployAndAddToFranklin(wallet, franklinDeployedContract);
    });
  
    it('Ether deposit', async () => {
        await etherLib.deposit("0.1", wallet, franklinDeployedContract);
    });
  
});
