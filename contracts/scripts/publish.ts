import { deployedAddressesFromEnv } from '../src.ts/deploy';
import { ethers } from 'ethers';

const hre = require('hardhat');

async function main() {
    if (process.env.CHAIN_ETH_NETWORK == 'localhost') {
        console.log('Skip contract publish on localhost');
        return;
    }
    const addresses = deployedAddressesFromEnv();
    for (const address of [
        addresses.ZkSyncTarget,
        addresses.VerifierTarget,
        addresses.GovernanceTarget,
        addresses.AdditionalZkSync
    ]) {
        try {
            await hre.run('verify:verify', { address });
        } catch (e) {
            console.error(e);
        }
    }

    {
        const address = addresses.UpgradeGatekeeper;
        const constructorArguments = [addresses.ZkSync];

        await hre.run('verify:verify', { address, constructorArguments });
    }

    {
        const address = addresses.ZkSync;
        const zkSyncEncodedArguments = ethers.utils.defaultAbiCoder.encode(
            ['address', 'address', 'bytes32'],
            [addresses.Governance, addresses.Verifier, process.env.CONTRACTS_GENESIS_ROOT]
        );

        const constructorArguments = [addresses.ZkSyncTarget, zkSyncEncodedArguments];

        try {
            await hre.run('verify:verify', { address, constructorArguments });
        } catch (e) {
            console.error(e);
        }
    }

    {
        const address = addresses.Governance;
        const governanceEncodedArguments = ethers.utils.defaultAbiCoder.encode(['address'], [addresses.DeployFactory]);

        const constructorArguments = [addresses.GovernanceTarget, governanceEncodedArguments];

        try {
            await hre.run('verify:verify', { address, constructorArguments });
        } catch (e) {
            console.error(e);
        }
    }

    {
        const address = addresses.Verifier;
        const verifierEncodedArguments = ethers.utils.defaultAbiCoder.encode([], []);

        const constructorArguments = [addresses.VerifierTarget, verifierEncodedArguments];

        try {
            await hre.run('verify:verify', { address, constructorArguments });
        } catch (e) {
            console.error(e);
        }
    }

    {
        const address = addresses.RegenesisMultisig;

        const constructorArguments = [process.env.MISC_REGENESIS_THRESHOLD];

        try {
            await hre.run('verify:verify', { address, constructorArguments });
        } catch (e) {
            console.error(e);
        }
    }

    {
        const address = addresses.NFTFactory;

        const name = process.env.NFT_FACTORY_NAME;
        const symbol = process.env.NFT_FACTORY_SYMBOL;
        const zksyncAddress = addresses.ZkSync;

        const constructorArguments = [name, symbol, zksyncAddress];

        try {
            await hre.run('verify:verify', { address, constructorArguments });
        } catch (e) {
            console.error(e);
        }
    }

    {
        const address = addresses.TokenGovernance;

        const governance = addresses.Governance;
        const listingFeeToken = process.env.MISC_LISTING_FEE_TOKEN;
        const listingFee = process.env.MISC_LISTING_FEE;
        const listingCap = process.env.MISC_LISTING_CAP;
        const treasury = process.env.MISC_LISTING_TREASURY;

        const constructorArguments = [governance, listingFeeToken, listingFee, listingCap, treasury];

        try {
            await hre.run('verify:verify', { address, constructorArguments });
        } catch (e) {
            console.error(e);
        }
    }
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
