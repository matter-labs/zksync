pragma solidity 0.5.10;

import "./Bytes.sol";


/// @title Primitives for manipulating Operations
/// @author Matter Labs
library Operations {

    uint8 constant ADDRESS_BYTES = 20;
    uint8 constant UINT16_BYTES = 2;
    uint8 constant UINT128_BYTES = 8;
    uint8 constant HASH_BYTES = 20;


    /// @notice Token id bytes lengths
    uint8 constant TOKEN_BYTES = 2;

    /// @notice Token amount bytes lengths
    uint8 constant AMOUNT_BYTES = 16;

    /// @notice Address bytes lengths
    uint8 constant ETH_ADDR_BYTES = 20;

    /// @notice Franklin chain address length
    uint8 constant PUBKEY_HASH_BYTES = 20;

    /// @notice Fee bytes lengths
    uint8 constant FEE_BYTES = 2;

    /// @notice Franklin account id bytes lengths
    uint8 constant ACC_NUM_BYTES = 3;

    /// @notice Franklin nonce bytes lengths
    uint8 constant NONCE_BYTES = 4;

    /// @notice Signature (for example full exit signature) bytes length
    uint8 constant SIGNATURE_BYTES = 64;

    /// @notice Public key bytes length
    uint8 constant PUBKEY_BYTES = 32;




    /// @notice Noop operation length
    uint256 constant NOOP_BYTES = 1 * 8;
    
    /// @notice Deposit operation length
    uint256 constant DEPOSIT_BYTES = 6 * 8;
    
    /// @notice Transfer to new operation length
    uint256 constant TRANSFER_TO_NEW_BYTES = 5 * 8;
    
    /// @notice Withdraw operation length
    uint256 constant PARTIAL_EXIT_BYTES = 6 * 8;
    
    /// @notice Close operation length
    uint256 constant CLOSE_ACCOUNT_BYTES = 1 * 8;
    
    /// @notice Transfer operation length
    uint256 constant TRANSFER_BYTES = 2 * 8;
    
    /// @notice Full exit operation length
    uint256 constant FULL_EXIT_BYTES = 18 * 8;



    /// @notice Types of franklin operations in blocks
    enum OpType {
        Noop,
        Deposit,
        TransferToNew,
        PartialExit,
        CloseAccount,
        Transfer,
        FullExit
    }

    uint256 constant DEPOSIT_PUBDATA_BYTES = ADDRESS_BYTES + UINT16_BYTES + UINT128_BYTES + HASH_BYTES;

    struct DepositPubdata {
        address owner;
        uint16 token;
        uint128 amount; 
        bytes pubkey_hash;
    }

    function readDepositPubdata(bytes memory _data, uint _offset) internal pure
        returns (uint offset, DepositPubdata memory parsed)
    {
        offset = _offset;
        bytes memory buf;

        (offset, buf) = Bytes.read(_data, offset, ETH_ADDR_BYTES);
        parsed.owner = Bytes.bytesToAddress(buf);
        (offset, buf) = Bytes.read(_data, offset, TOKEN_BYTES);
        parsed.token = Bytes.bytesToUInt16(buf);
        (offset, buf) = Bytes.read(_data, offset, AMOUNT_BYTES);
        parsed.amount = Bytes.bytesToUInt128(buf);
        (offset, buf) = Bytes.read(_data, offset, PUBKEY_HASH_BYTES);
        parsed.pubkey_hash = buf;
    }

    function writeDepositPubdata(DepositPubdata memory deposit) internal pure returns (bytes memory) {
        bytes memory buf = new bytes(DEPOSIT_PUBDATA_BYTES);
        uint offset = 0;

        offset = Bytes.memcpy(buf, offset, Bytes.toBytesFromAddress(deposit.owner));
        offset = Bytes.memcpy(buf, offset, Bytes.toBytesFromUInt16(deposit.token));
        offset = Bytes.memcpy(buf, offset, Bytes.toBytesFromUInt128(deposit.amount));
        offset = Bytes.memcpy(buf, offset, deposit.pubkey_hash);

        return buf;
    }


        // // Priority Queue request
        // bytes memory pubData = Bytes.toBytesFromUInt24(_accountId); // franklin id
        // pubData = Bytes.concat(pubData, _pubKey); // account id
        // pubData = Bytes.concat(pubData, Bytes.toBytesFromAddress(msg.sender)); // eth address
        // pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(tokenId)); // token id
        // pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt32(_nonce)); // nonce
        // pubData = Bytes.concat(pubData, _signature); // signature

        // priorityQueue.addPriorityRequest(uint8(OpType.FullExit), fee, pubData);
        

        // if (_opType == uint8(OpType.Deposit)) {
        //     bytes memory pubData = Bytes.slice(_publicData, opDataPointer + ACC_NUM_BYTES, TOKEN_BYTES + AMOUNT_BYTES + PUBKEY_HASH_BYTES);
        //     require(
        //         pubData.length == TOKEN_BYTES + AMOUNT_BYTES + PUBKEY_HASH_BYTES,
        //         "fpp11"
        //     ); // fpp11 - wrong deposit length
        //     onchainOps[_currentOnchainOp] = OnchainOperation(
        //         OpType.Deposit,
        //         pubData
        //     );
        //     return (DEPOSIT_BYTES, 1, 1);
        // }

        // if (_opType == uint8(OpType.PartialExit)) {
        //     bytes memory pubData = Bytes.slice(_publicData, opDataPointer + ACC_NUM_BYTES, TOKEN_BYTES + AMOUNT_BYTES + FEE_BYTES + ETH_ADDR_BYTES);
        //     require(
        //         pubData.length == TOKEN_BYTES + AMOUNT_BYTES + FEE_BYTES + ETH_ADDR_BYTES,
        //         "fpp12"
        //     ); // fpp12 - wrong partial exit length
        //     onchainOps[_currentOnchainOp] = OnchainOperation(
        //         OpType.PartialExit,
        //         pubData
        //     );
        //     return (PARTIAL_EXIT_BYTES, 1, 0);
        // }

        // if (_opType == uint8(OpType.FullExit)) {
        //     bytes memory pubData = Bytes.slice(_publicData, opDataPointer, ACC_NUM_BYTES + PUBKEY_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_BYTES + AMOUNT_BYTES);
        //     require(
        //         pubData.length == ACC_NUM_BYTES + PUBKEY_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_BYTES + AMOUNT_BYTES,
        //         "fpp13"
        //     ); // fpp13 - wrong full exit length
        //     onchainOps[_currentOnchainOp] = OnchainOperation(
        //         OpType.FullExit,
        //         pubData
        //     );
        //     return (FULL_EXIT_BYTES, 1, 1);
        // }

        // for (uint64 current = start; current < end; ++current) {
        //     OnchainOperation memory op = onchainOps[current];
        //     if (op.opType == OpType.PartialExit) {
        //         // partial exit was successful, accrue balance
        //         bytes memory tokenBytes = new bytes(TOKEN_BYTES);
        //         for (uint8 i = 0; i < TOKEN_BYTES; ++i) {
        //             tokenBytes[i] = op.pubData[i];
        //         }
        //         uint16 tokenId = Bytes.bytesToUInt16(tokenBytes);

        //         bytes memory amountBytes = new bytes(AMOUNT_BYTES);
        //         for (uint256 i = 0; i < AMOUNT_BYTES; ++i) {
        //             amountBytes[i] = op.pubData[TOKEN_BYTES + i];
        //         }
        //         uint128 amount = Bytes.bytesToUInt128(amountBytes);

        //         bytes memory ethAddress = new bytes(ETH_ADDR_BYTES);
        //         for (uint256 i = 0; i < ETH_ADDR_BYTES; ++i) {
        //             ethAddress[i] = op.pubData[TOKEN_BYTES + AMOUNT_BYTES + FEE_BYTES + i];
        //         }
        //         storeWithdrawalAsPending(Bytes.bytesToAddress(ethAddress), tokenId, amount);
        //     }
        //     if (op.opType == OpType.FullExit) {
        //         // full exit was successful, accrue balance
        //         bytes memory tokenBytes = new bytes(TOKEN_BYTES);
        //         for (uint8 i = 0; i < TOKEN_BYTES; ++i) {
        //             tokenBytes[i] = op.pubData[ACC_NUM_BYTES + PUBKEY_BYTES + ETH_ADDR_BYTES + i];
        //         }
        //         uint16 tokenId = Bytes.bytesToUInt16(tokenBytes);

        //         bytes memory amountBytes = new bytes(AMOUNT_BYTES);
        //         for (uint256 i = 0; i < AMOUNT_BYTES; ++i) {
        //             amountBytes[i] = op.pubData[ACC_NUM_BYTES + PUBKEY_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_BYTES + i];
        //         }
        //         uint128 amount = Bytes.bytesToUInt128(amountBytes);

        //         bytes memory ethAddress = new bytes(ETH_ADDR_BYTES);
        //         for (uint256 i = 0; i < ETH_ADDR_BYTES; ++i) {
        //             ethAddress[i] = op.pubData[ACC_NUM_BYTES + PUBKEY_BYTES + i];
        //         }
        //         storeWithdrawalAsPending(Bytes.bytesToAddress(ethAddress), tokenId, amount);
        //     }
        //     delete onchainOps[current];

}
