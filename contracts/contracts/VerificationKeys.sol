
// This contract is generated programmatically

pragma solidity ^0.4.24;
import "./DepositVerificationKey.sol";
import "./TransferVerificationKey.sol";
import "./ExitVerificationKey.sol";


// Hardcoded constants to avoid accessing store
contract VerificationKeys is TransferVerificationKey, DepositVerificationKey, ExitVerificationKey {
}
