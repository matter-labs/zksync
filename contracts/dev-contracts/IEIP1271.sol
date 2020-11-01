pragma solidity ^0.5.0;

interface IEIP1271 {
  /**
   * @dev Should return whether the signature provided is valid for the provided data
   * @param _hash Hash which was signed on the behalf of address(this)
   * @param _signature Signature byte array associated with _data
   *
   * MUST return the bytes4 magic value 0x1626ba7e when function passes.
   * MUST NOT modify state (using STATICCALL for solc < 0.5, view modifier for solc > 0.5)
   * MUST allow external calls
   */
  function isValidSignature(
    bytes32 _hash,
    bytes calldata _signature)
    external
    view
    returns (bytes4);
}
