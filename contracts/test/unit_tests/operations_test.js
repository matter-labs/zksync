const { expect } = require("chai")
const { provider, wallet, deployTestContract, getCallRevertReason } = require("./common")

describe("Operations unit test", function () {
    this.timeout(50000);

    let testContract
    before(async () => {
        testContract = await deployTestContract('../../build/OperationsTest')
    });

    // Deposit

    it("should convert Deposit pubdata", async () => {
        await testContract.testDeposit()
    });

    it("should return true when offchain and onchain Deposit pubdata match", async () => {
        let offchain = "0x" +
            "010203" +                                        // accountId -- not matching
            "0102" +                                          // tokenId
            "101112131415161718191a1b1c1d1e1f" +              // amount
            "823B747710C5bC9b8A47243f2c3d1805F1aA00c5" +      // owner
            "0100000000000000000000000000000000000002";       // pubkeyHash
        expect(await testContract.testDepositMatch(offchain)).to.equal(true)
    });

    it("should return false when offchain and onchain Deposit pubdata don't match", async () => {
        let offchain = "0x" +
            "010203" +                                        // accountId
            "0000" +                                          // tokenId -- not matching
            "101112131415161718191a1b1c1d1e1f" +              // amount
            "823B747710C5bC9b8A47243f2c3d1805F1aA00c5" +      // owner
            "0100000000000000000000000000000000000002";       // pubkeyHash
        expect(await testContract.testDepositMatch(offchain)).to.equal(false)

        offchain = "0x" +
            "010203" +                                        // accountId
            "0102" +                                          // tokenId
            "101112131415161718191a1b1c1d1e1f" +              // amount
            "823B747710C5bC9b8A47243f2c3d1805F1aA00c5" +      // owner
            "0100000000000000000000000000000000000003";       // pubkeyHash -- last byte not matching
        expect(await testContract.testDepositMatch(offchain)).to.equal(false)
    });

    // Full exit

    it("should convert FullExit pubdata", async () => {
        await testContract.testFullExit()
    });

    it("should return true when offchain and onchain FullExit pubdata match", async () => {
        let offchain = "0x" +
            "010203" +                                    // accountId
            "0000000000000000000000000000000000000000000000000000000000000000" +  // pubkey
            "823B747710C5bC9b8A47243f2c3d1805F1aA00c5" +  // owner
            "3132" +                                      // tokenId
            "41424344" +                                  // nonce
            "0000000000000000000000000000000000000000000000000000000000000000" +  // sig s
            "0000000000000000000000000000000000000000000000000000000000000000" +  // sig r
            "101112131415161718191a1b1c1d1e1f";           // amount -- not matching but should be ignored
        expect(await testContract.testFullExitMatch(offchain)).to.equal(true)
    });

    it("should return false when offchain and onchain FullExit pubdata match", async () => {
        let offchain = "0x" +
            "000203" +                                    // accountId -- not matching
            "0000000000000000000000000000000000000000000000000000000000000000" +  // pubkey
            "823B747710C5bC9b8A47243f2c3d1805F1aA00c5" +  // owner
            "3132" +                                      // tokenId
            "41424344" +                                  // nonce
            "0000000000000000000000000000000000000000000000000000000000000000" +  // sig s
            "0000000000000000000000000000000000000000000000000000000000000000" +  // sig r
            "101112131415161718191a1b1c1d1e1f";           // amount        
        expect(await testContract.testFullExitMatch(offchain)).to.equal(false)

        offchain = "0x" +
            "010203" +                                    // accountId -- not matching
            "0000000000000000000000000000000000000000000000000000000000000000" +  // pubkey
            "823B747710C5bC9b8A47243f2c3d1805F1aA00c5" +  // owner
            "3132" +                                      // tokenId
            "41424300" +                                  // nonce -- not matching
            "0000000000000000000000000000000000000000000000000000000000000000" +  // sig s
            "0000000000000000000000000000000000000000000000000000000000000000" +  // sig r
            "101112131415161718191a1b1c1d1e1f";           // amount        
        expect(await testContract.testFullExitMatch(offchain)).to.equal(false)        
    });

    // Parital exit

    it("should convert PartialExit pubdata", async () => {
        await testContract.testPartialExit()
    });

});
