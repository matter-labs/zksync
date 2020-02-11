pragma solidity 0.5.16;

import "../Bytes.sol";


contract BytesTest {

    function concat(bytes calldata _a, bytes calldata _b) external pure returns (bytes memory) {
        return Bytes.concat(_a, _b);
    }

    function read(bytes calldata _data, uint _offset, uint _len) external pure returns (uint new_offset, bytes memory data) {
        return Bytes.read(_data, _offset, _len);
    }
}

