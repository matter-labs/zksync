import {ethers} from "ethers";
import {bigNumberify, parseEther, hexlify} from "ethers/utils";
import {
    signersTestContractCode,
    deploySigners
} from "../src.ts/deploy";

import {expect, use, assert} from "chai";
import {solidity} from "ethereum-waffle";

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const signerAddress = "0809101112131415161718192021222334252627";

describe("VERIFIER", function() {
    this.timeout(50000);

    const message = '0x7b0a2020226f70656e223a207b0a20202020227072696365223a2039353931372c0a202020202274696d65223a207b0a20202020202022756e6978223a20313438333134323430302c0a2020202020202269736f223a2022323031362d31322d33315430303a30303a30302e3030305a220a202020207d0a20207d2c0a202022636c6f7365223a207b0a20202020227072696365223a2039363736302c0a202020202274696d65223a207b0a20202020202022756e6978223a20313438333232383830302c0a2020202020202269736f223a2022323031372d30312d30315430303a30303a30302e3030305a220a202020207d0a20207d2c0a2020226c6f6f6b7570223a207b0a20202020227072696365223a2039363736302c0a20202020226b223a20312c0a202020202274696d65223a207b0a20202020202022756e6978223a20313438333232383830302c0a2020202020202269736f223a2022323031372d30312d30315430303a30303a30302e3030305a220a202020207d0a20207d0a7d0a6578616d706c652e636f6d2f6170692f31';
    const sigX = '11181692345848957662074290878138344227085597134981019040735323471731897153462';
    const sigY = '6479746447046570360435714249272776082787932146211764251347798668447381926167';

    beforeEach(async () => {
        console.log("---\n");
    });

    it("Signers full test", async () => {
        console.log("\n - Signers add started");
        const signersDeployedContract = await deploySigners(wallet, wallet.address, 51, signersTestContractCode);

        const result1 = await signersDeployedContract.addSigner(
            signerAddress,
            '18523194229674161632574346342370534213928970227736813349975332190798837787897',
            '5725452645840548248571879966249653216818629536104756116202892528545334967238',
            '3816656720215352836236372430537606984911914992659540439626020770732736710924',
            '677280212051826798882467475639465784259337739185938192379192340908771705870'
        );
        await result1.wait();
        
        expect(await signersDeployedContract.signersCount()).equal(1);

        const result2 = await signersDeployedContract.isSigner(signerAddress);
        expect(result2).to.eq(true);

        console.log("\n + Signers add passed");

        console.log("\n - Changing min sigs percentage started");

        const result3 = await signersDeployedContract.changeMinSigsPercentage(60);
        await result3.wait();

        expect(await signersDeployedContract.minSigsPercentage()).equal(60);

        console.log("\n + Changing min sigs percentage passed");
        
        console.log("\n - Verifying message started");

        const aggrPubKey = await signersDeployedContract.aggregatePubKeys(
            [signerAddress]
        );

        const aggrSignature = await signersDeployedContract.aggregateSignatures(
            [sigX, sigY], 1
        );

        // const h1 = await signersDeployedContract.messageToG11(
        //     message
        // );

        // console.log(h1);

        // const h2 = await signersDeployedContract.messageToG12(
        //     message
        // );

        // console.log(h2);

        const result4 = await signersDeployedContract.verify(
            aggrSignature[0],
            aggrSignature[1],
            aggrPubKey[0],
            aggrPubKey[1],
            aggrPubKey[2],
            aggrPubKey[3],
            message,
        );
        expect(result4).to.eq(true);

        console.log("\n + Verifying message passed");
    });

});