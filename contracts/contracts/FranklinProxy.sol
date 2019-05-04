pragma solidity ^0.4.24;

import {FranklinCommon} from "./common/FranklinCommon.sol";

contract FranklinProxy is FranklinCommon {
    // Well, technically it's not :)

    constructor(address _depositor, address _transactor, address _exitor) public {
        nextAccountToRegister = 2;
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        depositor = _depositor;
        transactor = _transactor;
        exitor = _exitor;

    }

    function deposit(uint256[2] memory, uint128) public payable {
        callExternal(depositor);
    }

    function depositInto(uint24, uint128) public payable {
        callExternal(depositor);
    }

    function cancelDeposit() public {
        callExternal(depositor);
    }

    function startNextDepositBatch() public {
        callExternal(depositor);
    }

    function changeDepositBatchFee(uint128) public {
        callExternal(depositor);
    }

    function commitDepositBlock(uint256, uint24[DEPOSIT_BATCH_SIZE] memory, uint32, bytes32) public {
        callExternal(depositor);
    }

    function verifyDepositBlock(uint256, uint24[DEPOSIT_BATCH_SIZE] memory, uint32, uint256[8] memory) public {
        callExternal(depositor);
    } 

    function commitTransferBlock(uint32, uint128, bytes memory, bytes32) public {
        callExternal(transactor);
    }

    function verifyTransferBlock(uint32, uint256[8] memory) public {
        callExternal(transactor);
    }

    function exit() public payable {
        callExternal(exitor);
    }

    function cancelExit() public {
        callExternal(exitor);
    }

    function startNextExitBatch() public {
        callExternal(exitor);
    }

    function changeExitBatchFee(uint128) public {
        callExternal(exitor);
    }

    function commitExitBlock(uint256, uint24[EXIT_BATCH_SIZE] memory, uint32, bytes memory, bytes32) public {
        callExternal(exitor);
    }

    function verifyExitBlock(uint256, uint32, uint256[8] memory) public {
        callExternal(exitor);
    }

    function withdrawUserBalance(uint256) public {
        callExternal(exitor);
    }

    // this is inline delegate-call to dispatch functions to subcontracts that are responsible for execution
    function callExternal(address callee) internal {
        assembly {
            let memoryPointer := mload(0x40)
            calldatacopy(memoryPointer, 0, calldatasize)
            let newFreeMemoryPointer := add(memoryPointer, calldatasize)
            mstore(0x40, newFreeMemoryPointer)
            let retVal := delegatecall(sub(gas, 2000), callee, memoryPointer, calldatasize, newFreeMemoryPointer, 0x40)
            let retDataSize := returndatasize
            returndatacopy(newFreeMemoryPointer, 0, retDataSize)
            switch retVal case 0 { revert(newFreeMemoryPointer, returndatasize) } default { return(newFreeMemoryPointer, retDataSize) }
            //return(newFreeMemoryPointer, retDataSize)
        }
    }
}