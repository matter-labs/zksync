import {deployContract} from 'ethereum-waffle';
import {ethers} from 'ethers';
import {bigNumberify} from "ethers/utils";
import Axios from "axios";
const qs = require('querystring');

export const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();
export const franklinContractCode = require('../build/Franklin');
export const vkContractCode = require('../build/VerificationKey');
export const verifierContractCode = require('../build/Verifier');
export const governanceContractCode = require('../build/Governance');

export async function depoloyGovernance(
    wallet,
    governanceCode = governanceContractCode
    ) {
    try {
        let governance = await deployContract(wallet, governanceCode, [], {
            gasLimit: 3000000,
        });
        console.log(`GOVERNANCE_ADDR=${governance.address}`);

        return governance
    } catch (err) {
        console.log("Governance deploy error:" + err);
    }
}

export async function deployFranklin(
    wallet,
    governanceAddress,
    genesisRoot = ethers.constants.HashZero,
    franklinCode = franklinContractCode,
    verifierCode = verifierContractCode,
    vkCode = vkContractCode
    ) {
    try {
        let verifier = await deployContract(wallet, verifierCode, [], {
            gasLimit: 1000000,
        });
        let vk = await deployContract(wallet, vkCode, [], {
            gasLimit: 1000000,
        });
        let contract = await deployContract(
            wallet,
            franklinCode,
            [
                verifier.address,
                vk.address,
                genesisRoot,
                ethers.constants.AddressZero,
                wallet.address,
                governanceAddress
            ],
        {
            gasLimit: 6500000,
        });
        console.log(`FRANKLIN_ADDR=${contract.address}`);

        return contract
    } catch (err) {
        console.log("Franklin deploy error:" + err);
    }
}

export async function postContractToTesseracts(contractCode, contractName: string, address: string) {
    let req = {
        contract_source: JSON.stringify(contractCode.abi),
        contract_compiler: "abi-only",
        contract_name: contractName,
        contract_optimized: false
    };

    const config = {
        headers: {
            'Content-Type': 'application/x-www-form-urlencoded'
        }
    };
    await Axios.post(`http://localhost:8000/${address}/contract`, qs.stringify(req), config);
}

export async function addTestERC20Token(wallet, governance) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, bigNumberify("1000000000"));
        console.log("TEST_ERC20=" + erc20.address);
        await (await governance.addToken(erc20.address)).wait();
        return erc20
    } catch (err) {
        console.log("Add token error:" + err);
    }
}
