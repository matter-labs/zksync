pragma solidity ^0.5.0;

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
        FullExit,
        ChangePubKey
    }

    // Byte lengths

    uint8 constant TOKEN_BYTES = 2;

    uint8 constant PUBKEY_BYTES = 32;

    uint8 constant NONCE_BYTES = 4;

    uint8 constant PUBKEY_HASH_BYTES = 20;

    uint8 constant ADDRESS_BYTES = 20;

    /// @notice Packed fee bytes lengths
    uint8 constant FEE_BYTES = 2;

    /// @notice zkSync account id bytes lengths
    uint8 constant ACCOUNT_ID_BYTES = 3;

    uint8 constant AMOUNT_BYTES = 16;

    /// @notice Signature (for example full exit signature) bytes length
    uint8 constant SIGNATURE_BYTES = 64;

    // Deposit pubdata

    struct Deposit {
        // uint24 accountId -- present in pubdata, ignored at serialization
        uint16 tokenId;
        uint128 amount; 
        address owner;
    }

    uint public constant PACKED_DEPOSIT_PUBDATA_BYTES = 
        ACCOUNT_ID_BYTES + TOKEN_BYTES + AMOUNT_BYTES + ADDRESS_BYTES;

    /// Deserialize deposit pubdata
    function readDepositPubdata(bytes memory _data, uint _offset) internal pure
        returns (Deposit memory parsed)
    {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = _offset + ACCOUNT_ID_BYTES;                   // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner

        require(offset == _offset + PACKED_DEPOSIT_PUBDATA_BYTES, "rdp10"); // reading invalid deposit pubdata size
    }

    /// Serialize deposit pubdata
    function writeDepositPubdata(Deposit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            new bytes(ACCOUNT_ID_BYTES),          // accountId (ignored)
            Bytes.toBytesFromUInt16(op.tokenId),  // tokenId
            Bytes.toBytesFromUInt128(op.amount),  // amount
            Bytes.toBytesFromAddress(op.owner)    // owner
        );
    }

    /// @notice Check that deposit pubdata from request and block matches
    function depositPubdataMatch(bytes memory _lhs, bytes memory _rhs) internal pure returns (bool) {
        // We must ignore `accountId` because it is present in block pubdata but not in priority queue
        bytes memory lhs_trimmed = Bytes.slice(_lhs, ACCOUNT_ID_BYTES, PACKED_DEPOSIT_PUBDATA_BYTES - ACCOUNT_ID_BYTES);
        bytes memory rhs_trimmed = Bytes.slice(_rhs, ACCOUNT_ID_BYTES, PACKED_DEPOSIT_PUBDATA_BYTES - ACCOUNT_ID_BYTES);
        return keccak256(lhs_trimmed) == keccak256(rhs_trimmed);
    }

    // FullExit pubdata

    struct FullExit {
        uint24 accountId;
        address owner;
        uint16 tokenId;
        uint128 amount;
    }

    uint public constant PACKED_FULL_EXIT_PUBDATA_BYTES = 
        ACCOUNT_ID_BYTES + ADDRESS_BYTES + TOKEN_BYTES + AMOUNT_BYTES;

    function readFullExitPubdata(bytes memory _data) internal pure
        returns (FullExit memory parsed)
    {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = 0;
        (offset, parsed.accountId) = Bytes.readUInt24(_data, offset);      // accountId
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);         // owner
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset);        // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset);        // amount
    }

    function writeFullExitPubdata(FullExit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            Bytes.toBytesFromUInt24(op.accountId),  // accountId
            Bytes.toBytesFromAddress(op.owner),     // owner
            Bytes.toBytesFromUInt16(op.tokenId),    // tokenId
            Bytes.toBytesFromUInt128(op.amount)     // amount
        );
    }

    /// @notice Check that full exit pubdata from request and block matches
    function fullExitPubdataMatch(bytes memory _lhs, bytes memory _rhs) internal pure returns (bool) {
        // We must ignore `amount` because it is present in block pubdata but not in priority queue
        require(PACKED_FULL_EXIT_PUBDATA_BYTES - AMOUNT_BYTES == 25, "edm11");  // expected PACKED_FULL_EXIT_PUBDATA_BYTES - AMOUNT_BYTES to be 25
        require(_lhs.length >= 25, "edm12");  // _lhs too short
        require(_rhs.length >= 25, "edm13");  // _rhs too short

        bool equals;
        assembly {
            let a := mload(add(_lhs, 0x20))
            let b := mload(add(_rhs, 0x20))
            equals := eq(shr(56, a), shr(56, b)) // discard 32 - 25 = 7 bytes (56 bits)
        }

        return equals;
    }

    // PartialExit pubdata
    
    struct PartialExit {
        //uint24 accountId; -- present in pubdata, ignored at serialization
        uint16 tokenId;
        uint128 amount;
        //uint16 fee; -- present in pubdata, ignored at serialization
        address owner;
    }

    function readPartialExitPubdata(bytes memory _data) internal pure
        returns (PartialExit memory parsed)
    {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = ACCOUNT_ID_BYTES;                             // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES;                                        // fee (ignored)
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner
    }

    function writePartialExitPubdata(PartialExit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            new bytes(ACCOUNT_ID_BYTES),          // accountId (ignored)
            Bytes.toBytesFromUInt16(op.tokenId),  // tokenId
            Bytes.toBytesFromUInt128(op.amount),  // amount
            new bytes(FEE_BYTES),                 // fee (ignored)
            Bytes.toBytesFromAddress(op.owner)    // owner
        );
    }

    // ChangePubKey

    struct ChangePubKey {
        // uint24 accountId; -- present in pubdata, ignored at serialization
        bytes20 pubKeyHash;
        address owner;
        uint32 nonce;
    }

    function readChangePubKeyPubdata(bytes memory _data, uint _offset) internal pure
        returns (ChangePubKey memory parsed)
    {
        require(PUBKEY_HASH_BYTES == 20, "rcp11"); // expected PUBKEY_HASH_BYTES to be 20

        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = _offset + ACCOUNT_ID_BYTES;                                    // accountId (ignored)
        (offset, parsed.pubKeyHash) = Bytes.readBytes20(_data, offset);              // pubKeyHash
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);                   // owner
        (offset, parsed.nonce) = Bytes.readUInt32(_data, offset);                    // nonce
    }

}
