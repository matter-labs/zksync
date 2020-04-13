pragma solidity 0.5.16;


library Bytes {

    function toBytesFromUInt16(uint16 self) internal pure returns (bytes memory _bts) {
        return toBytesFromUIntTruncated(uint(self), 2);
    }

    function toBytesFromUInt24(uint24 self) internal pure returns (bytes memory _bts) {
        return toBytesFromUIntTruncated(uint(self), 3);
    }

    function toBytesFromUInt32(uint32 self) internal pure returns (bytes memory _bts) {
        return toBytesFromUIntTruncated(uint(self), 4);
    }

    function toBytesFromUInt128(uint128 self) internal pure returns (bytes memory _bts) {
        return toBytesFromUIntTruncated(uint(self), 16);
    }

    // Copies 'len' lower bytes from 'self' into a new 'bytes memory'.
    // Returns the newly created 'bytes memory'. The returned bytes will be of length 'len'.
    function toBytesFromUIntTruncated(uint self, uint8 byteLength) private pure returns (bytes memory bts) {
        require(byteLength <= 32, "bt211");
        bts = new bytes(byteLength);
        // Even though the bytes will allocate a full word, we don't want
        // any potential garbage bytes in there.
        uint data = self << ((32 - byteLength) * 8);
        assembly {
            mstore(add(bts, /*BYTES_HEADER_SIZE*/32), data)
        }
    }

    // Copies 'self' into a new 'bytes memory'.
    // Returns the newly created 'bytes memory'. The returned bytes will be of length '20'.
    function toBytesFromAddress(address self) internal pure returns (bytes memory bts) {
        bts = toBytesFromUIntTruncated(uint(self), 20);
    }

    function bytesToAddress(bytes memory self) internal pure returns (address addr) {
        require(self.length == 20, "bbs11");
        assembly {
            // Load 32 bytes, address is only 20 bytes, discard 12 bytes (96 bits)
            addr := shr(96, mload(add(self, 0x20)))
        }
    }

    function bytesToUInt16(bytes memory self) internal pure returns (uint16 r) {
        require(self.length >= 2, "bb611");
        assembly {
            r := mload(add(add(self, 0x2), 0))
        }
    }

    function bytesToUInt24(bytes memory self) internal pure returns (uint24 r) {
        require(self.length >= 3, "bb411");
        assembly {
            r := mload(add(add(self, 0x3), 0))
        }
    }

    function bytesToUInt32(bytes memory self) internal pure returns (uint32 r) {
        require(self.length >= 4, "bb411");
        assembly {
            r := mload(add(add(self, 0x4), 0))
        }
    }

    function bytesToUInt128(bytes memory self) internal pure returns (uint128 r)
    {
        require(self.length >= 16, "bb811");
        assembly {
            r := mload(add(add(self, 0x10), 0))
        }
    }

    function bytesToBytes32(bytes memory  _input) internal pure returns (bytes32 _output) {
        require (_input.length == 0x20);
        assembly {
            _output := mload(add(_input, 0x20))
        }
    }

    // Original source code: https://github.com/GNSPS/solidity-bytes-utils/blob/master/contracts/BytesLib.sol#L228
    // Get slice from bytes arrays
    // Returns the newly created 'bytes memory'
    function slice(
        bytes memory _bytes,
        uint _start,
        uint _length
    )
        internal
        pure
        returns (bytes memory)
    {
        require(_bytes.length >= (_start + _length), "bse11"); // bytes length is less then start byte + length bytes

        bytes memory tempBytes = new bytes(_length);

        if (_length != 0) {
            // TODO: Review this thoroughly.
            assembly {
                let slice_curr := add(tempBytes, 0x20)
                let slice_end := add(slice_curr, _length)

                for {
                    // The multiplication in the next line has the same exact purpose
                    // as the one above.
                    let array_current := add(_bytes, add(_start, 0x20))
                } lt(slice_curr, slice_end) {
                    slice_curr := add(slice_curr, 0x20)
                    array_current := add(array_current, 0x20)
                } {
                    mstore(slice_curr, mload(array_current))
                }
            }
        }

        return tempBytes;
    }

    /// Reads byte stream
    /// @return new_offset - offset + amount of bytes read
    /// @return data - actually read data
    function read(bytes memory _data, uint _offset, uint _length) internal pure returns (uint new_offset, bytes memory data) {
        data = slice(_data, _offset, _length);
        new_offset = _offset + _length;
    }

    function readUInt16(bytes memory _data, uint _offset) internal pure returns (uint new_offset, uint16 r) {
        bytes memory buf;
        (new_offset, buf) = read(_data, _offset, 2);
        r = bytesToUInt16(buf);
    }

    function readUInt24(bytes memory _data, uint _offset) internal pure returns (uint new_offset, uint24 r) {
        bytes memory buf;
        (new_offset, buf) = read(_data, _offset, 3);
        r = bytesToUInt24(buf);
    }

    function readUInt32(bytes memory _data, uint _offset) internal pure returns (uint new_offset, uint32 r) {
        bytes memory buf;
        (new_offset, buf) = read(_data, _offset, 4);
        r = bytesToUInt32(buf);
    }

    function readUInt128(bytes memory _data, uint _offset) internal pure returns (uint new_offset, uint128 r) {
        bytes memory buf;
        (new_offset, buf) = read(_data, _offset, 16);
        r = bytesToUInt128(buf);
    }

    function readAddress(bytes memory _data, uint _offset) internal pure returns (uint new_offset, address r) {
        bytes memory buf;
        (new_offset, buf) = read(_data, _offset, 20);
        r = bytesToAddress(buf);
    }

    // Helper function for hex conversion.
    function halfByteToHex(byte _byte) internal pure returns (byte _hexByte) {
        uint8 numByte = uint8(_byte);
        if (numByte >= 0 && numByte <= 9) {
            return byte(0x30 + numByte); // ASCII 0-9
        } else if (numByte <= 15) {
            return byte(0x57 + numByte); // ASCII a-f
        }
    }

    // Convert bytes to ASCII hex representation
    function bytesToHexASCIIBytes(bytes memory  _input) internal pure returns (bytes memory _output) {
        bytes memory outStringBytes = new bytes(_input.length * 2);
        for (uint i = 0; i < _input.length; ++i) {
            outStringBytes[i*2] = halfByteToHex(_input[i] >> 4);
            outStringBytes[i*2+1] = halfByteToHex(_input[i] & 0x0f);
        }
        return outStringBytes;
    }

}
