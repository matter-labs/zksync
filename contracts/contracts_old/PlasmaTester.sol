pragma solidity ^0.4.24;

import {PlasmaTransactor} from "./PlasmaTransactor.sol";
import {PlasmaDepositor} from "./PlasmaDepositor.sol";
import {PlasmaExitor} from "./PlasmaExitor.sol";

contract PlasmaTester is PlasmaDepositor, PlasmaExitor, PlasmaTransactor {
    uint256 constant DEPOSIT_BATCH_SIZE = 1;

    uint24 constant operatorsAccounts = 4;
    uint24 public nextAccountToRegister = operatorsAccounts;

    // create technological accounts for an operator. 
    constructor(uint256[operatorsAccounts - 1] memory defaultPublicKeys) public {
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        // account number 0 is NEVER registered
        Account memory freshAccount;
        for (uint24 i = 1; i < operatorsAccounts; i++) {
            freshAccount = Account(
                uint8(AccountState.REGISTERED),
                uint32(0),
                msg.sender,
                defaultPublicKeys[i-1],
                uint32(0),
                uint32(0)
            );
            accounts[i] = freshAccount;
        }
    }

    function verifyProof(Circuit, uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid)
    {
        return true;
    }
}