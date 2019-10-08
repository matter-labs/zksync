pragma solidity ^0.5.8;

import "./BlsOperations.sol";

contract BlsOpsTester {

    BlsOperations.G2Point verificationKey;

    constructor() public {
        verificationKey = BlsOperations.G2Point({
            x: [
                18523194229674161632574346342370534213928970227736813349975332190798837787897,
                5725452645840548248571879966249653216818629536104756116202892528545334967238
            ],
            y: [
                3816656720215352836236372430537606984911914992659540439626020770732736710924,
                677280212051826798882467475639465784259337739185938192379192340908771705870
            ]
        });
    }

    function testVerify(bytes memory _message, uint _sigX, uint _sigY) public view returns (bool) {
        BlsOperations.G1Point memory signature = BlsOperations.G1Point({
            x: _sigX,
            y: _sigY
        });
        BlsOperations.G1Point memory messageHash = BlsOperations.messageToG1(_message);
        return BlsOperations.twoPointsPairing(BlsOperations.negate(signature), messageHash, BlsOperations.generatorG2(), verificationKey);
    }
}