

const TwistedEdwards = artifacts.require("TwistedEdwards");
const assert = require("assert");

contract('Plasma', async (accounts) => {
    const BN = require("bn.js");

    const account = accounts[0];
    let contract;
    
    beforeEach(async () => {
        let x = new BN("2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7", 16);
        let y = new BN("05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7", 16);
        contract = await TwistedEdwards.new([x, y], {from: account})
    })

    it('check generator on curve', async () => {
        try {
            let x = new BN("2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7", 16);
            let y = new BN("05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7", 16);
            
            let generatorIsCorrect = await contract.checkOnCurve([x, y]);
            assert(generatorIsCorrect, "generator is not on curve");

            let gasEstimate = await contract.checkOnCurve.estimateGas([x, y]);
            console.log("Checking a point is on curve takes gas = " + gasEstimate);

        } catch(error) {
            console.log(error);
            throw error;
        }
    })

    it('check generator order', async () => {
        try {
            let x = new BN("2ef3f9b423a2c8c74e9803958f6c320e854a1c1c06cd5cc8fd221dc052d76df7", 16);
            let y = new BN("05a01167ea785d3f784224644a68e4067532c815f5f6d57d984b5c0e9c6c94b7", 16);
            
            let generatorIsCorrect = await contract.isCorrectGroup([x, y]);
            assert(generatorIsCorrect, "generator is not in correct group");

            let gasEstimate = await contract.isCorrectGroup.estimateGas([x, y]);
            console.log("Checking a point order takes gas = " + gasEstimate);

        } catch(error) {
            console.log(error);
            throw error;
        }
    })

    
});
