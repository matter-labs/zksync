pragma solidity ^0.5.0;

contract Plasma {

    enum Circuit {
        DEPOSIT,
        UPDATE,
        WITHDRAWAL
    }

    // for testing purposes only owner may submit proofs
    address public owner;

    constructor() public {
        owner = msg.sender;
    }

    modifier restricted() {
        if (msg.sender == owner) _;
    }

    // Verifier stub
    function verify(Circuit /*circuit*/, uint256[8] memory /*proof*/, uint256[] memory /*inputs*/)
        internal pure returns (bool valid)
    {
        return true;
    }



}
