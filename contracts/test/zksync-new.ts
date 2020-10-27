import { ethers } from "hardhat";
import { Signer } from "ethers";
import { ZkSync} from "../typechain";

describe("Token", function () {
    let accounts: Signer[];
    let zkSync: ZkSync;

    beforeEach(async function () {
        accounts = await ethers.getSigners();
        const zksyncFactory = await ethers.getContractFactory("ZkSync")
        const deployed = await zksyncFactory.deploy();
        zkSync.attach(deployed.address);
    });

    it("commit", async function () {

    });
});