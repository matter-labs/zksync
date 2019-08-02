import {ethers} from 'ethers';
import {deployFranklin, addTestERC20Token} from "../src.ts/deploy";
const chai = require('chai');
const {solidity} = require('ethereum-waffle');


chai.use(solidity);
const {expect} = chai;

const franklinContractCode = require('../build/Franklin');
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

describe('INTEGRATION: Deposit', function () {
    this.timeout(10000);

    let franklinDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        franklinDeployedContract = await deployFranklin(wallet);
        erc20DeployedToken = await addTestERC20Token(wallet, franklinDeployedContract);
    });

    it('Ether deposit', async () => {
        // TODO:
        // Use franklin_lib for testing
    });

});
