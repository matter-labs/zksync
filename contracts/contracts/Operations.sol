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

    // Byte lengths

    /// @notice Public key
    uint8 constant PUBKEY_BYTES = 32;

    /// @notice Public key
    uint8 constant PUBKEY_HASH_BYTES = 20;

    /// @notice Packed fee bytes lengths
    uint8 constant FEE_BYTES = 2;

    /// @notice zkSync account id bytes lengths
    uint8 constant ACCOUNT_ID_BYTES = 3;

    /// @notice Token amount bytes lengths
    uint8 constant AMOUNT_BYTES = 16;

    /// @notice Signature (for example full exit signature) bytes length
    uint8 constant SIGNATURE_BYTES = 64;

    // Deposit pubdata

    struct Deposit {
        // uint24 accountId
        uint16 tokenId;
        uint128 amount; 
        address owner;
        bytes pubkeyHash;
    }

    /// Deserialize deposit pubdata
    function readDepositPubdata(bytes memory _data, uint _offset) internal pure
        returns (uint new_offset, Deposit memory parsed)
    {
        uint offset = _offset + ACCOUNT_ID_BYTES;                   // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner
        (offset, parsed.pubkeyHash) = Bytes.read(_data, offset, PUBKEY_HASH_BYTES); // pubkey hash
        new_offset = _offset + ACCOUNT_ID_BYTES + 2 + 16 + 20 + PUBKEY_HASH_BYTES;
    }

    /// Serialize deposit pubdata
    function writeDepositPubdata(Deposit memory op) internal pure returns (bytes memory buf) {
        buf = new bytes(ACCOUNT_ID_BYTES);                             // accountId (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt16(op.tokenId));  // tokenId
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt128(op.amount));  // amount
        buf = Bytes.concat(buf, Bytes.toBytesFromAddress(op.owner));   // owner
        buf = Bytes.concat(buf, op.pubkeyHash);                        // pubkey hash
    }

    /// @notice Check that deposit pubdata from request and block matches
    function depositPubdataMatch(bytes memory _lhs, bytes memory _rhs) internal pure returns (bool) {
        // We must ignore `accountId` because it is present in block pubdata but not in priority queue
        bytes memory lhs_trimmed = Bytes.slice(_lhs, ACCOUNT_ID_BYTES, _lhs.length - ACCOUNT_ID_BYTES);
        bytes memory rhs_trimmed = Bytes.slice(_rhs, ACCOUNT_ID_BYTES, _rhs.length - ACCOUNT_ID_BYTES);
        return keccak256(lhs_trimmed) == keccak256(rhs_trimmed);
    }

    // FullExit pubdata

    struct FullExit {
        uint24 accountId;
        bytes pubkey;
        address owner;
        uint16 tokenId;
        uint32 nonce; 
        //bytes[64] signature (NOTE: to be removed soon by dvush anyway)
        uint128 amount;
    }

    function readFullExitPubdata(bytes memory _data, uint _offset) internal pure
        returns (FullExit memory parsed)
    {
        uint offset = _offset;
        (offset, parsed.accountId) = Bytes.readUInt24(_data, offset);      // accountId
        (offset, parsed.pubkey) = Bytes.read(_data, offset, PUBKEY_BYTES); // pubkey
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);         // owner
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset);        // tokenId
        (offset, parsed.nonce) = Bytes.readUInt32(_data, offset);          // nonce
        offset += SIGNATURE_BYTES;                                         // signature (ignored)
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset);        // amount
    }

    function writeFullExitPubdata(FullExit memory op) internal pure returns (bytes memory buf) {
        buf = Bytes.toBytesFromUInt24(op.accountId);                    // accountId
        buf = Bytes.concat(buf, op.pubkey);                             // pubkey
        buf = Bytes.concat(buf, Bytes.toBytesFromAddress(op.owner));    // owner
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt16(op.tokenId));   // tokenId
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt32(op.nonce));     // nonce
        buf = Bytes.concat(buf, new bytes(SIGNATURE_BYTES));            // signature (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt128(op.amount));   // amount
    }

    /// @notice Check that full exit pubdata from request and block matches
    function fullExitPubdataMatch(bytes memory _lhs, bytes memory _rhs) internal pure returns (bool) {
        // We must ignore `amount` because it is present in block pubdata but not in priority queue
        bytes memory lhs_trimmed = Bytes.slice(_lhs, 0, _lhs.length - AMOUNT_BYTES);
        bytes memory rhs_trimmed = Bytes.slice(_rhs, 0, _rhs.length - AMOUNT_BYTES);
        return keccak256(lhs_trimmed) == keccak256(rhs_trimmed);
    }

    // PartialExit pubdata
    
    struct PartialExit {
        //uint24 accountId;
        uint16 tokenId;
        uint128 amount;
        //uint16 fee;
        address owner;
    }

    function readPartialExitPubdata(bytes memory _data, uint _offset) internal pure
        returns (PartialExit memory parsed)
    {
        uint offset = _offset + ACCOUNT_ID_BYTES;                   // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES;                                        // fee (ignored)
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner
    }

    function writePartialExitPubdata(PartialExit memory op) internal pure returns (bytes memory buf) {
        buf = new bytes(ACCOUNT_ID_BYTES);                              // accountId (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt16(op.tokenId));   // tokenId
        buf = Bytes.concat(buf, Bytes.toBytesFromUInt128(op.amount));   // amount
        buf = Bytes.concat(buf, new bytes(FEE_BYTES));                  // fee (ignored)
        buf = Bytes.concat(buf, Bytes.toBytesFromAddress(op.owner));    // owner
    }

}
