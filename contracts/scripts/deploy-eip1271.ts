// This script deploys a test "smart wallet" which supports EIP-1271 signatures.
// Owner account address is taken from the `$ZKSYNC_HOME/etc/test_config/eip1271.json`.
// Deployed contract address is updated in the same file.

import { ethers } from "ethers";
import { readContractCode } from "../src.ts/deploy";
import { deployContract } from "ethereum-waffle";
import * as fs from "fs";

(async () => {
    if (!["test", "localhost"].includes(process.env.ETH_NETWORK)) {
        console.error("This deploy script is only for localhost-test network");
        process.exit(1);
    }

    const test_config_path = `${process.env.ZKSYNC_HOME}/etc/test_config/constant/eip1271.json`;
    const test_config = JSON.parse(fs.readFileSync(test_config_path, { encoding: "utf-8" }));

    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    provider.pollingInterval = 10;

    const deployWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);
    const smartWallet = await deployContract(
        deployWallet,
        readContractCode("AccountMock"),
        [test_config.owner_address],
        {
            gasLimit: 5000000,
        }
    );

    const out_config = {
        contract_address: smartWallet.address,
    };
    const out_config_path = `${process.env.ZKSYNC_HOME}/etc/test_config/volatile/eip1271.json`;
    fs.writeFileSync(out_config_path, JSON.stringify(out_config), { encoding: "utf-8" });
    process.exit(0);
})();
