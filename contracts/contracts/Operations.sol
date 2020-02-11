pragma solidity 0.5.16;

import "./Bytes.sol";


/// @title zkSync operations tools
library Operations {

    // Circuit ops and their pubdata (chunks * bytes)

    /// @notice zkSync circuit operation type
    enum OpType {
        Noop,
        Deposit,
        TransferToNew,
        PartialExit,
        CloseAccount,
        Transfer,
        FullExit
    }

    uint256 constant NOOP_BYTES = 1 * 8;
    uint256 constant DEPOSIT_BYTES = 6 * 8;
    uint256 constant TRANSFER_TO_NEW_BYTES = 5 * 8;
    uint256 constant PARTIAL_EXIT_BYTES = 6 * 8;
    uint256 constant CLOSE_ACCOUNT_BYTES = 1 * 8;
    uint256 constant TRANSFER_BYTES = 2 * 8;
    uint256 constant FULL_EXIT_BYTES = 18 * 8;

    // Byte lengths

    /// @notice TODO: obsolete (to remove)
    uint8 constant PUBKEY_HASH_BYTES = 20;

    /// @notice Packed fee bytes lengths
    uint8 constant FEE_BYTES = 2;

    /// @notice zkSync account id bytes lengths
    uint8 constant ACCOUNT_ID_BYTES = 3;

    // Deposit pubdata

    struct DepositPubdata {
        // uint24 accountId
        uint16 tokenId;
        uint128 amount; 
        address owner;
    }

    function readDepositPubdata(bytes memory _data, uint _offset) internal pure
        returns (DepositPubdata memory parsed)
    {
        uint offset = _offset + ACCOUNT_ID_BYTES;                   // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner
    }

    function writeDepositPubdata(DepositPubdata memory deposit) internal pure returns (bytes memory buf) {
        buf = new bytes(ACCOUNT_ID_BYTES);                                  // accountId (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt16(deposit.tokenId));  // tokenId
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt128(deposit.amount));  // amount
        buf = Bytes.concat(buf, Bytes.toBytesFromAddress(deposit.owner));   // owner
    }

    // FullExit pubdata

    struct FullExitPubdata {
        uint24 accountId;
        bytes pubkeyHash;
        address owner;
        uint16 tokenId;
        uint32 nonce; 
    }

    function readFullExitPubdata(bytes memory _data, uint _offset) internal pure
        returns (FullExitPubdata memory parsed)
    {
        uint offset = _offset + ACCOUNT_ID_BYTES;                                   // accountId (ignored)
        (offset, parsed.pubkeyHash) = Bytes.read(_data, offset, PUBKEY_HASH_BYTES); // pubkeyHash
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);                  // owner
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset);                 // tokenId
        (offset, parsed.nonce) = Bytes.readUInt32(_data, offset);                   // nonce
    }

    function writeFullExitPubdata(FullExitPubdata memory op) internal pure returns (bytes memory buf) {
        buf = new bytes(ACCOUNT_ID_BYTES);                              // accountId (ignored)
        buf = Bytes.concat(buf, op.pubkeyHash);                         // pubkeyHash
        buf = Bytes.concat(buf, Bytes.toBytesFromAddress(op.owner));    // owner
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt16(op.tokenId));   // tokenId
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt32(op.nonce));     // nonce
    }

    // PartialExit pubdata
    
    struct PartialExitPubdata {
        //uint24 accountId;
        uint16 tokenId;
        uint128 amount;
        //uint16 fee;
        address owner;
    }

    function readPartialExitPubdata(bytes memory _data, uint _offset) internal pure
        returns (PartialExitPubdata memory parsed)
    {
        uint offset = _offset + ACCOUNT_ID_BYTES;                   // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES;                                        // fee (ignored)
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner
    }

    function writePartialExitPubdata(PartialExitPubdata memory op) internal pure returns (bytes memory buf) {
        buf = new bytes(ACCOUNT_ID_BYTES);                              // accountId (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt16(op.tokenId));   // tokenId
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt128(op.amount));   // amount
        buf = Bytes.concat(buf, new bytes(FEE_BYTES));                  // fee (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromAddress(op.owner));    // owner
    }

}
