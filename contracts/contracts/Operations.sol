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
        _CloseAccount, // used for correct op id offset
        Transfer,
        FullExit,
        ChangePubKey,
        ForcedExit
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
    uint8 constant ACCOUNT_ID_BYTES = 4;

    uint8 constant AMOUNT_BYTES = 16;

    /// @notice Signature (for example full exit signature) bytes length
    uint8 constant SIGNATURE_BYTES = 64;

    // Deposit pubdata
    struct Deposit {
        uint32 accountId;
        uint16 tokenId;
        uint128 amount;
        address owner;
    }

    uint public constant PACKED_DEPOSIT_PUBDATA_BYTES = 
        ACCOUNT_ID_BYTES + TOKEN_BYTES + AMOUNT_BYTES + ADDRESS_BYTES;

    /// Deserialize deposit pubdata
    function readDepositPubdata(bytes memory _data) internal pure
        returns (Deposit memory parsed)
    {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = 0;
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset); // accountId
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset);   // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset);   // amount
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);    // owner

        require(offset == PACKED_DEPOSIT_PUBDATA_BYTES, "rdp10"); // reading invalid deposit pubdata size
    }

    /// Serialize deposit pubdata
    function writeDepositPubdata(Deposit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            bytes4(0),   // accountId (ignored) (update when ACCOUNT_ID_BYTES is changed)
            op.tokenId,  // tokenId
            op.amount,   // amount
            op.owner     // owner
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
        uint32 accountId;
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
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset);      // accountId
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);         // owner
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset);        // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset);        // amount

        require(offset == PACKED_FULL_EXIT_PUBDATA_BYTES, "rfp10"); // reading invalid full exit pubdata size
    }

    function writeFullExitPubdata(FullExit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            op.accountId,  // accountId
            op.owner,      // owner
            op.tokenId,    // tokenId
            op.amount      // amount
        );
    }

    /// @notice Check that full exit pubdata from request and block matches
    function fullExitPubdataMatch(bytes memory _lhs, bytes memory _rhs) internal pure returns (bool) {
        // `amount` is ignored because it is present in block pubdata but not in priority queue
        uint lhs = Bytes.trim(_lhs, PACKED_FULL_EXIT_PUBDATA_BYTES - AMOUNT_BYTES);
        uint rhs = Bytes.trim(_rhs, PACKED_FULL_EXIT_PUBDATA_BYTES - AMOUNT_BYTES);
        return lhs == rhs;
    }

    // PartialExit pubdata
    
    struct PartialExit {
        //uint32 accountId; -- present in pubdata, ignored at serialization
        uint16 tokenId;
        uint128 amount;
        //uint16 fee; -- present in pubdata, ignored at serialization
        address owner;
    }

    function readPartialExitPubdata(bytes memory _data, uint _offset) internal pure
        returns (PartialExit memory parsed)
    {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = _offset + ACCOUNT_ID_BYTES;                   // accountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES;                                        // fee (ignored)
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);  // owner
    }

    function writePartialExitPubdata(PartialExit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            bytes4(0),  // accountId (ignored) (update when ACCOUNT_ID_BYTES is changed)
            op.tokenId, // tokenId
            op.amount,  // amount
            bytes2(0),  // fee (ignored)  (update when FEE_BYTES is changed)
            op.owner    // owner
        );
    }

    // ForcedExit pubdata
    
    struct ForcedExit {
        //uint32 initiatorAccountId; -- present in pubdata, ignored at serialization
        //uint32 targetAccountId; -- present in pubdata, ignored at serialization
        uint16 tokenId;
        uint128 amount;
        //uint16 fee; -- present in pubdata, ignored at serialization
        address target;
    }

    function readForcedExitPubdata(bytes memory _data, uint _offset) internal pure
        returns (ForcedExit memory parsed)
    {
        // NOTE: there is no check that variable sizes are same as constants (i.e. TOKEN_BYTES), fix if possible.
        uint offset = _offset + ACCOUNT_ID_BYTES * 2;               // initiatorAccountId + targetAccountId (ignored)
        (offset, parsed.tokenId) = Bytes.readUInt16(_data, offset); // tokenId
        (offset, parsed.amount) = Bytes.readUInt128(_data, offset); // amount
        offset += FEE_BYTES;                                        // fee (ignored)
        (offset, parsed.target) = Bytes.readAddress(_data, offset); // target
    }

    function writeForcedExitPubdata(ForcedExit memory op) internal pure returns (bytes memory buf) {
        buf = abi.encodePacked(
            bytes4(0),  // initiatorAccountId (ignored) (update when ACCOUNT_ID_BYTES is changed)
            bytes4(0),  // targetAccountId (ignored) (update when ACCOUNT_ID_BYTES is changed)
            op.tokenId, // tokenId
            op.amount,  // amount
            bytes2(0),  // fee (ignored)  (update when FEE_BYTES is changed)
            op.target   // target
        );
    }

    // ChangePubKey

    struct ChangePubKey {
        uint32 accountId;
        bytes20 pubKeyHash;
        address owner;
        uint32 nonce;
        //uint16 tokenId; -- present in pubdata, ignored at serialization
        //uint16 fee; -- present in pubdata, ignored at serialization
    }

    function readChangePubKeyPubdata(bytes memory _data, uint _offset) internal pure
        returns (ChangePubKey memory parsed)
    {
        uint offset = _offset;
        (offset, parsed.accountId) = Bytes.readUInt32(_data, offset);                // accountId
        (offset, parsed.pubKeyHash) = Bytes.readBytes20(_data, offset);              // pubKeyHash
        (offset, parsed.owner) = Bytes.readAddress(_data, offset);                   // owner
        (offset, parsed.nonce) = Bytes.readUInt32(_data, offset);                    // nonce
    }

    // Withdrawal data process

    function readWithdrawalData(bytes memory _data, uint _offset) internal pure
        returns (bool _addToPendingWithdrawalsQueue, address _to, uint16 _tokenId, uint128 _amount)
    {
        uint offset = _offset;
        (offset, _addToPendingWithdrawalsQueue) = Bytes.readBool(_data, offset);
        (offset, _to) = Bytes.readAddress(_data, offset);
        (offset, _tokenId) = Bytes.readUInt16(_data, offset);
        (offset, _amount) = Bytes.readUInt128(_data, offset);
    }

}
