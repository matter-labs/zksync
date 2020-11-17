pragma solidity ^0.5.8;

import "../contracts/Bytes.sol";

contract BytesTest {
    function read(
        bytes calldata _data,
        uint256 _offset,
        uint256 _len
    ) external pure returns (uint256 new_offset, bytes memory data) {
        return Bytes.read(_data, _offset, _len);
    }

    function testUInt24(uint24 x) external pure returns (uint24 r, uint256 offset) {
        require(keccak256(new bytes(0)) == keccak256(new bytes(0)));

        bytes memory buf = Bytes.toBytesFromUInt24(x);
        (offset, r) = Bytes.readUInt24(buf, 0);
    }

    function bytesToHexConvert(bytes calldata _in) external pure returns (string memory) {
        return string(Bytes.bytesToHexASCIIBytes(_in));
    }
}
