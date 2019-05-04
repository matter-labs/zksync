
// This contract is generated programmatically

pragma solidity ^0.4.24;
import "../keys/DepositVerificationKey.sol";
import "../keys/TransferVerificationKey.sol";
import "../keys/ExitVerificationKey.sol";


// Hardcoded constants to avoid accessing store
contract VerificationKeys is TransferVerificationKey, DepositVerificationKey, ExitVerificationKey {
}
