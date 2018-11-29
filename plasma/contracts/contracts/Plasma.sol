pragma solidity ^0.5.0;

contract Plasma {

    // For testing purposes: only single operator may submit new blocks
    address public operator;

    constructor() public {
        operator = msg.sender;
    }

    // Public API

    // Deposit ERC20 tokens
    function deposit(address /*_from*/, uint /*_amount*/) public {

    }



    // Implementation

    enum Circuit {
        DEPOSIT,
        UPDATE,
        WITHDRAWAL
    }

    // Verifier stub: this part with hardcoded keys will be generated from Groth16 CRS
    function verify(Circuit /*circuit*/, uint256[8] memory /*proof*/, uint256[] memory /*inputs*/)
        internal pure returns (bool valid)
    {
        return true;
    }



}
