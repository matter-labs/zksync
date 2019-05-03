pragma solidity ^0.4.24;

import {Plasma} from "./Plasma.sol";
import {PlasmaDepositor} from "./PlasmaDepositor.sol";
import {PlasmaTransactor} from "./PlasmaTransactor.sol";
import {PlasmaExitor} from "./PlasmaExitor.sol";

contract FranklinProxy is Plasma {
    // Well, technically it's not :)

    constructor(address _depositor, address _transactor, address _exitor, uint256[2] _paddingPubKey) public {
        nextAccountToRegister = 2;
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        depositor = _depositor;
        transactor = _transactor;
        exitor = _exitor;

        // make the first deposit to install pub_key for padding
        deposit(_paddingPubKey, 0);
    }

    function deposit(uint256[2] memory publicKey, uint128 maxFee) public payable 
    {
        PlasmaDepositor(depositor).deposit(publicKey, maxFee);
    }

    function depositInto(uint24 accountID, uint128 maxFee) public payable
    {
        PlasmaDepositor(depositor).depositInto(accountID, maxFee);
    }

    function cancelDeposit() public 
    {
        PlasmaDepositor(depositor).cancelDeposit();
    }

    function startNextDepositBatch() public {
        PlasmaDepositor(depositor).startNextDepositBatch();
    }

    function changeDepositBatchFee(uint128 newBatchFee) public  
    {
        PlasmaDepositor(depositor).changeDepositBatchFee(newBatchFee);
    }
    function commitDepositBlock(
        uint256 batchNumber,
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs,
        uint32 blockNumber, 
        bytes32 newRoot
    ) public 
    {
        PlasmaDepositor(depositor).commitDepositBlock(batchNumber, accoundIDs, blockNumber, newRoot);
    }

    function verifyDepositBlock(
        uint256 batchNumber, 
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) public
    {
        PlasmaDepositor(depositor).verifyDepositBlock(batchNumber, accoundIDs, blockNumber, proof);
    } 

    function commitTransferBlock(
        uint32 blockNumber, 
        uint128 totalFees, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) public 
    {
        PlasmaTransactor(transactor).commitTransferBlock(blockNumber, totalFees, txDataPacked, newRoot);
    }

    function verifyTransferBlock(uint32 blockNumber, uint256[8] memory proof) public 
    {
        PlasmaTransactor(transactor).verifyTransferBlock(blockNumber, proof);
    }


    function exit() public payable 
    {
        PlasmaExitor(exitor).exit();
    }

    function cancelExit() public
    {
        PlasmaExitor(exitor).cancelExit();
    }

    function startNextExitBatch() public 
    {
        PlasmaExitor(exitor).startNextExitBatch();
    }

    function changeExitBatchFee(uint128 newBatchFee) public 
    {
        PlasmaExitor(exitor).changeExitBatchFee(newBatchFee);
    }

    function commitExitBlock(
        uint256 batchNumber,
        uint24[EXIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) public 
    {
        PlasmaExitor(exitor).commitExitBlock(batchNumber, accoundIDs, blockNumber, txDataPacked, newRoot);
    }

    function verifyExitBlock(
        uint256 batchNumber, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) public 
    {
        PlasmaExitor(exitor).verifyExitBlock(batchNumber, blockNumber, proof);
    }

    function withdrawUserBalance(
        uint256 iterationsLimit
    ) public 
    {
        PlasmaExitor(exitor).withdrawUserBalance(iterationsLimit);
    }

}