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
export const verifierContractCode = require('../build/Verifier');
export const governanceContractCode = require('../build/Governance');
export const priorityQueueContractCode = require('../build/PriorityQueue');
// export const signersContractCode = require('../build/Signers');

export const franklinTestContractCode = require('../build/FranklinTest');
export const verifierTestContractCode = require('../build/VerifierTest');
export const governanceTestContractCode = require('../build/GovernanceTest');
export const priorityQueueTestContractCode = require('../build/PriorityQueueTest');
// export const signersTestContractCode = require('../build/SignersTest');

export const comptrollerContractCode = require('../build/Unitroller');
export const interestRateModelContractCode = require('../build/WhitePaperInterestRateModel');
export const cetherContractCode = require('../build/CEther');
export const cecr20ContractCode = require('../build/CErc20');

export async function deployComptroller(
    wallet,
    comptrollerCode
    ) {
    try {
        let comptroller = await deployContract(wallet, comptrollerCode, [], {
            gasLimit: 8000000,
        });
        console.log(`COMPTROLLER_ADDR=${comptroller.address}`);

        return comptroller
    } catch (err) {
        console.log("Comptroller deploy error:" + err);
    }
}

export async function deployInterestRateModel(
    wallet,
    baseRate,
    multiplier,
    interestRateModelCode
    ) {
    try {
        let interestRateModel = await deployContract(wallet, interestRateModelCode, [baseRate, multiplier], {
            gasLimit: 8000000,
        });
        console.log(`INTEREST_RATE_MODEL_ADDR=${interestRateModel.address}`);

        return interestRateModel
    } catch (err) {
        console.log("InterestRateModel deploy error:" + err);
    }
}

export async function deployCEther(
    wallet,
    comptrollerAddress,
    interestRateModerAddress,
    initialExchangeRateMantissa,
    name,
    symbol,
    decimals,
    adminAddress,
    cetherCode
    ) {
    try {
        let cether = await deployContract(
            wallet,
            cetherCode,
            [
                comptrollerAddress,
                interestRateModerAddress,
                initialExchangeRateMantissa,
                name,
                symbol,
                decimals,
                adminAddress
            ], 
            { gasLimit: 8000000 }
        );
        console.log(`CETHER_ADDR=${cether.address}`);

        return cether
    } catch (err) {
        console.log("CEther deploy error:" + err);
    }
}

export async function deployCErc20(
    wallet,
    underlyingAddress,
    comptrollerAddress,
    interestRateModerAddress,
    initialExchangeRateMantissa,
    name,
    symbol,
    decimals,
    adminAddress,
    cerc20Code
    ) {
    try {
        let cerc20 = await deployContract(
            wallet,
            cerc20Code,
            [
                underlyingAddress,
                comptrollerAddress,
                interestRateModerAddress,
                initialExchangeRateMantissa,
                name,
                symbol,
                decimals,
                adminAddress
            ],
            { gasLimit: 8000000 }
        );
        console.log(`CERC20_ADDR=${cerc20.address}`);

        return cerc20
    } catch (err) {
        console.log("CErc20 deploy error:" + err);
    }
}

export async function deployGovernance(
    wallet,
    governorAddress,
    governanceCode
    ) {
    try {
        let governance = await deployContract(wallet, governanceCode, [governorAddress], {
            gasLimit: 3000000,
        });
        console.log(`GOVERNANCE_ADDR=${governance.address}`);

        return governance
    } catch (err) {
        console.log("Governance deploy error:" + err);
    }
}

export async function deployPriorityQueue(
    wallet,
    ownerAddress,
    priorityQueueCode
) {
    try {
        let priorityQueue = await deployContract(wallet, priorityQueueCode, [ownerAddress], {
            gasLimit: 5000000,
        });
        console.log(`PRIORITY_QUEUE_ADDR=${priorityQueue.address}`);

        return priorityQueue
    } catch (err) {
        console.log("Priority queue deploy error:" + err);
    }
}

export async function deployVerifier(
    wallet,
    verifierCode
) {
    try {
        let verifier = await deployContract(wallet, verifierCode, [], {
            gasLimit: 2000000,
        });
        console.log(`VERIFIER_ADDR=${verifier.address}`);

        return verifier
    } catch (err) {
        console.log("Verifier deploy error:" + err);
    }
}

export async function deployFranklin(
    franklinCode,
    wallet,
    governanceAddress,
    priorityQueueAddress,
    verifierAddress,
    genesisAddress,
    genesisRoot = ethers.constants.HashZero
) {
    try {
        let contract = await deployContract(
            wallet,
            franklinCode,
            [
                governanceAddress,
                verifierAddress,
                priorityQueueAddress,
                genesisAddress,
                genesisRoot,
            ],
            {
                gasLimit: 6600000,
            });
        console.log(`CONTRACT_ADDR=${contract.address}`);

        const priorityQueueContract = new ethers.Contract(priorityQueueAddress, priorityQueueContractCode.interface, wallet);
        await (await priorityQueueContract.changeFranklinAddress(contract.address)).wait();
        return contract
    } catch (err) {
        console.log("Franklin deploy error:" + err);
    }
}

// export async function deploySigners(
//     wallet,
//     ownerAddress,
//     minSigsPercent,
//     signersCode,
// ) {
//     try {
//         let contract = await deployContract(
//             wallet,
//             signersCode,
//             [
//                 ownerAddress,
//                 minSigsPercent
//             ],
//             {
//                 gasLimit: 6600000,
//             });
//         console.log(`SIGNERS_CONTRACT_ADDR=${contract.address}`);

//         return contract
//     } catch (err) {
//         console.log("Signers deploy error:" + err);
//     }
// }

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
    await Axios.post(`${process.env.TESSERACTS_URL}/${address}/contract`, qs.stringify(req), config);
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

export async function addTestNotApprovedERC20Token(wallet) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, bigNumberify("1000000000"));
        console.log("TEST_ERC20=" + erc20.address);
        return erc20
    } catch (err) {
        console.log("Add token error:" + err);
    }
}
