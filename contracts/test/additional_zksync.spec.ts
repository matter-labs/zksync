import { expect } from 'chai';
import * as hardhat from 'hardhat';
import { Interface } from 'ethers/lib/utils';

export function getAllSelectors(contractInterface: Interface) {
    return Object.keys(contractInterface.functions).map((signature) => contractInterface.getSighash(signature));
}

describe('Additional ZkSync tests', function () {
    const additionalZkSyncInterface = new Interface(hardhat.artifacts.readArtifactSync('AdditionalZkSync').abi);
    const zksyncInterface = new Interface(hardhat.artifacts.readArtifactSync('ZkSync').abi);

    it('zkSync contract contains all additional zkSync contract functions', async () => {
        const zksyncSelectors = getAllSelectors(zksyncInterface);
        const additionalZksyncSelectors = getAllSelectors(additionalZkSyncInterface);
        expect(zksyncSelectors).to.include.members(additionalZksyncSelectors);
    });
});
