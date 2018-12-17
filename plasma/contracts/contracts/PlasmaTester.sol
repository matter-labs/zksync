pragma solidity ^0.4.24;

import {PlasmaContract} from "./PlasmaContract.sol";

contract PlasmaTester is PlasmaContract {

    function verifyProof(Circuit, uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid)
    {
        return true;
    }
}