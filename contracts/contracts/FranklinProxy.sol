pragma solidity ^0.4.24;

import {Plasma} from "./Plasma.sol";
import {PlasmaDepositor} from "./PlasmaDepositor.sol";
import {PlasmaTransactor} from "./PlasmaTransactor.sol";
import {PlasmaExitor} from "./PlasmaExitor.sol";

contract FranklinProxy is Plasma {
    // Well, technically it's not :)

    constructor(address _depositor, address _transactor, address _exitor) public {
        nextAccountToRegister = 2;
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        depositor = _depositor;
        transactor = _transactor;
        exitor = _exitor;

        // make the first deposit to install pub_key for padding
        // deposit(_paddingPubKey, 0);
    }

    function deposit(uint256[2] memory publicKey, uint128 maxFee) public payable 
    {
        callExternal(depositor);
    }

    function depositInto(uint24 accountID, uint128 maxFee) public payable
    {
        callExternal(depositor);
    }

    function cancelDeposit() public 
    {
        callExternal(depositor);
    }

    function startNextDepositBatch() public {
        callExternal(depositor);
    }

    function changeDepositBatchFee(uint128 newBatchFee) public  
    {
        callExternal(depositor);
    }
    function commitDepositBlock(
        uint256 batchNumber,
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs,
        uint32 blockNumber, 
        bytes32 newRoot
    ) public 
    {
        callExternal(depositor);
    }

    function verifyDepositBlock(
        uint256 batchNumber, 
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) public
    {
        callExternal(depositor);
    } 

    function commitTransferBlock(
        uint32 blockNumber, 
        uint128 totalFees, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) public 
    {
        callExternal(transactor);
    }

    function verifyTransferBlock(uint32 blockNumber, uint256[8] memory proof) public 
    {
        callExternal(transactor);
    }


    function exit() public payable 
    {
        callExternal(exitor);
    }

    function cancelExit() public
    {
        callExternal(exitor);
    }

    function startNextExitBatch() public 
    {
        callExternal(exitor);
    }

    function changeExitBatchFee(uint128 newBatchFee) public 
    {
        callExternal(exitor);
    }

    function commitExitBlock(
        uint256 batchNumber,
        uint24[EXIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) public 
    {
        callExternal(exitor);
    }

    function verifyExitBlock(
        uint256 batchNumber, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) public 
    {
        callExternal(exitor);
    }

    function withdrawUserBalance(
        uint256 iterationsLimit
    ) public 
    {
        callExternal(exitor);
    }

    function callExternal(address callee) internal {
        assembly {
            let memoryPointer := mload(0x40)
            calldatacopy(memoryPointer, 0, calldatasize)
            let newFreeMemoryPointer := add(memoryPointer, calldatasize)
            mstore(0x40, newFreeMemoryPointer)
            let retVal := delegatecall(sub(gas, 2000), callee, memoryPointer, calldatasize, newFreeMemoryPointer, 0x40)
            let retDataSize := returndatasize
            returndatacopy(newFreeMemoryPointer, 0, retDataSize)
            switch retVal case 0 { revert(0,0) } default { return(newFreeMemoryPointer, retDataSize) }
        }
    }
}