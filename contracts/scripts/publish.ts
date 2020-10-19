import { Wallet } from "ethers";
import { Deployer } from "../src.ts/deploy";

(async () => {
    try {
        // Wallet is not needed for publishing
        const deployer = new Deployer({ deployWallet: Wallet.createRandom() });
        if (process.env.ETH_NETWORK === "localhost") {
            await deployer.publishSourcesToTesseracts();
        } else {
            await deployer.publishSourcesToEtherscan();
        }
        process.exit(0);
    } catch (e) {
        console.error("Failed to publish contracts code:", e.toString());
    }
})();
