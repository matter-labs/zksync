pragma solidity ^0.5.1;

contract DummyVerifier {
    function Verify(
        uint256[14] memory,
        uint256[] memory,
        uint256[8] memory,
        uint256[] memory
    ) internal view returns (bool) {
        return true;
    }
}
