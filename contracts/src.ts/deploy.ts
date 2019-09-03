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
export const priorityQueueContractCode = require('../build/PriorityQueue');

export async function deployFranklin(
    wallet,
    genesisRoot = ethers.constants.HashZero,
    franklinCode = franklinContractCode,
    verifierCode = verifierContractCode,
    vkCode = vkContractCode,
    priorityQueueCode = priorityQueueContractCode
    ) {
    try {
        let verifier = await deployContract(wallet, verifierCode, [], {
            gasLimit: 1000000,
        });
        let vk = await deployContract(wallet, vkCode, [], {
            gasLimit: 1000000,
        });
        let priorityQueue = await deployContract(wallet, priorityQueueCode, [], {
            gasLimit: 8000000,
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
                priorityQueue.address
            ],
        {
            gasLimit: 8000000,
        });
        console.log(`CONTRACT_ADDR=${contract.address}`);

        return contract
    } catch (err) {
        console.log("Error:" + err);
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

export async function addTestERC20Token(wallet, franklin) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, bigNumberify("1000000000"));
        console.log("TEST_ERC20=" + erc20.address);
        await (await franklin.addToken(erc20.address)).wait();
        return erc20
    } catch (err) {
        console.log("Error:" + err);
    }
}
