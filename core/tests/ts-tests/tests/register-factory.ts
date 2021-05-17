import { deployContract } from 'ethereum-waffle';
import fs from "fs";
import {Tester} from "./tester";
import {utils} from "ethers";
import {expect} from "chai";
import { Wallet, types, wallet } from 'zksync';

function readContractCode(name: string) {
    const fileName = name.split('/').pop();
    return JSON.parse(
        fs.readFileSync(`../../../contracts/artifacts/cache/solpp-generated-contracts/${name}.sol/${fileName}.json`, { encoding: 'utf-8' })
    );
}
function readFactoryCode() {
    return readContractCode("ZkSyncNFTCustomFactory")
}

declare module './tester' {
    interface Tester {
        testRegisterFactory(
            wallet: Wallet,
        ): Promise<Address>;
    }
}

Tester.prototype.testRegisterFactory = async function (
    wallet: Wallet,
) {
    wallet.provider.contractAddress.mainContract

    const contract = await deployContract(wallet, readFactoryCode(),["TestFactory", "TS", wallet.provider.contractAddress.mainContract,
            wallet.provider.contractAddress.govContract
        ],
        {
            gasLimit: 5000000
        }
    )
    const {signature, accountId, accountAddress} = await wallet.signRegisterFactory(contract.address);
    await contract.registerFactory(accountId, accountAddress, signature);

    // mint nft and withdraw
};

