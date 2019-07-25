const Plasma = artifacts.require("Plasma");
const assert = require("assert");

contract('Plasma', async (accounts) => {
    const BN = require("bn.js");

    const account = accounts[0];
    let contract;
    
    beforeEach(async () => {
        contract = await Plasma.new({from: account})
    })

    it('commit to data', async () => {
        try {
            let publicData = "0x00000080000000be0000000080000000be0000000080000000be0000000080000000be0000000080000000be0000000080000000be0000000080000000be0000000080000000be00";
            let nextRoot = "0x1facb2cc667c5d3e7162274c00881fb98b2f5bf1c80fd7a612c7d7f2ca811089"
            let result = await contract.commitBlock(0, 0, publicData, nextRoot);
            let block = await contract.blocks(0);
            let totalCommitted = await contract.totalCommitted();
            console.log("Total commited = " + totalCommitted);

            let proof = [
                new BN("16755890309709504255050985180817557075102043093245672893842987730500160692655"),
                new BN("17971101070761193284039286941506202506127198560851924391966482795354105619809"),
                new BN("4572095663635183615127149738886689560505627507490525050282444962500344069475"),
                new BN("15157278983069442488620677124413686978990457609776312356413739423327009119236"),
                new BN("17880186821198566711513284459389214912525477464363278607518585813877553130748"),
                new BN("10255002830203696592186441422789589545615773753711791040005597942198369865646"),
                new BN("14023986121275820632410270476556337277250001417755645438870438964029440399619"),
                new BN("11871408088467689433052310116470249687042273778375592692006275805793257751339"),
            ]

            let proofResult = await contract.verifyBlock(0, proof);
            console.log("In verification previous root = " + proofResult.logs[0].args.a.toString(16));
            console.log("In verification new root = " + proofResult.logs[1].args.a.toString(16));
            console.log("In verification data commitment root = " + proofResult.logs[2].args.a.toString(16));
            console.log("Proof verificaiton success = " + proofResult.logs[3].args.b);
            let totalVerified = await contract.totalVerified();
            let lastVerifiedRoot = await contract.lastVerifiedRoot();
            assert(lastVerifiedRoot == nextRoot);

        } catch(error) {
            console.log(error);
            throw error;
        }
    })

    
});
