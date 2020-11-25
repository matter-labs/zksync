import {deployedAddressesFromEnv} from "../src.ts/deploy";

const hre = require("hardhat");

async function main() {
    if (process.env.ETH_NETWORK == 'localhost') {
        console.log("Skip contract publish on localhost");
        return;
    }
    const addresses = deployedAddressesFromEnv();
    for (const address of [addresses.ZkSyncTarget, addresses.VerifierTarget, addresses.GovernanceTarget]) {
        try {
            await hre.run('verify', {address});
        } catch (e) {
            console.error(e)
        }
    }
}

main()
    .then(() => process.exit(0))
    .catch(error => {
        console.error(error);
        process.exit(1);
    });
