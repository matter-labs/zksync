const Plasma = artifacts.require("PlasmaDepositor");
const assert = require("assert");
const transactionLib = require("../lib/transaction");
const ethUtils = require("ethereumjs-util");
const BN = require("bn.js");

console.log("Contract size = " + (Plasma.bytecode.length - 2)/2 );

const operatorsAccounts = 4;

const proof = [
    new BN("16755890309709504255050985180817557075102043093245672893842987730500160692655"),
    new BN("17971101070761193284039286941506202506127198560851924391966482795354105619809"),
    new BN("4572095663635183615127149738886689560505627507490525050282444962500344069475"),
    new BN("15157278983069442488620677124413686978990457609776312356413739423327009119236"),
    new BN("17880186821198566711513284459389214912525477464363278607518585813877553130748"),
    new BN("10255002830203696592186441422789589545615773753711791040005597942198369865646"),
    new BN("14023986121275820632410270476556337277250001417755645438870438964029440399619"),
    new BN("11871408088467689433052310116470249687042273778375592692006275805793257751339"),
]

contract('Plasma', async (accounts) => {


    const account = accounts[0];
    let contract;
    
    beforeEach(async () => {
        const accs = [];
        for (let i = 1; i < operatorsAccounts; i++) {
            const {packedPublicKey} = transactionLib.newKey();
            accs.push(packedPublicKey);
        }
        
        contract = await Plasma.new({from: account})
        console.log("Contract address = " + contract.address);
    })

    function randomPublicDataPiece() {
        let from = new BN(Math.floor(Math.random() * 1000000));
        let to = new BN(Math.floor(Math.random() * 1000000));
        let amount = new BN(Math.floor(Math.random() * 1000));
        let fee = new BN(Math.floor(Math.random() * 100));
        return transactionLib.getPublicData({from, to, amount, fee});
    }

    function randomExitDataPiece(account, exitAmount) {
        let from = new BN(account);
        let to = new BN(0);
        let amount = new BN(exitAmount);
        let fee = new BN(Math.floor(Math.random() * 100));
        return transactionLib.getPublicData({from, to, amount, fee}).bytes;
    }

    function randomPublicData(numTXes) {
        const arr = [];
        for (let i = 0; i < numTXes; i++) {
            arr.push(randomPublicDataPiece().bytes);
        }
        return Buffer.concat(arr);
    }

    it('do a deposit', async () => {
        try {
            let x = new BN("2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7", 16);
            let y = new BN("05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7", 16);

            let result = await contract.deposit([x, y], 0, {from: account, value: "1000000000000000000"});
            console.log(result);
            let total = await contract.totalDepositRequests();
            console.log("total requests = " + total.toString(10));
        } catch(error) {
            console.log(error);
            throw error;
        }
    })    
});
