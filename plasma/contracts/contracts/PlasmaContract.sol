pragma solidity ^0.4.24;

import {PlasmaDepositor} from "./PlasmaDepositor.sol";

contract PlasmaContract is PlasmaDepositor {
    // uint24 constant operatorsAccounts = 4;
    // uint24 public nextAccountToRegister = operatorsAccounts;

    // // create technological accounts for an operator. 
    // constructor(uint256[operatorsAccounts - 1] memory defaultPublicKeys, address _transactor, address _exitor) public {
    //     lastVerifiedRoot = EMPTY_TREE_ROOT;
    //     operators[msg.sender] = true;
    //     // account number 0 is NEVER registered
    //     Account memory freshAccount;
    //     for (uint24 i = 1; i < operatorsAccounts; i++) {
    //         freshAccount = Account(
    //             uint8(AccountState.REGISTERED),
    //             0,
    //             msg.sender,
    //             defaultPublicKeys[i-1]
    //         );
    //         accounts[i] = freshAccount;
    //     }
    //     transactor = _transactor;
    //     exitor = _exitor;
    // }

    constructor(address _transactor, address _exitor) public {
        nextAccountToRegister = 2;
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        transactor = _transactor;
        exitor = _exitor;
    }
}