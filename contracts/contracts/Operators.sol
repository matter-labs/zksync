pragma solidity ^0.5.8;
pragma experimental ABIEncoderV2;

import "./BlsOperations.sol";

contract Operators {

    address internal ownerAddress;

    uint256 internal validOperatorsMinimalPercentage;

    struct Operator {
        bool exists;
        BlsOperations.G2Point pubKey;
    }

    struct Signature {
        address operator;
        BlsOperations.G1Point signature;
    }

    uint256 public operatorsCount = 0;
    mapping (address => Operator) public operators;

    constructor(address _ownerAddress, uint256 _validOperatorsMinimalPercentage) public {
        require(
            _validOperatorsMinimalPercentage <= 100 && _validOperatorsMinimalPercentage > 0,
            "oscr11"
        ); // osar11 - we need operators percentage be between 0% and 100%
        ownerAddress = _ownerAddress;
        _validOperatorsMinimalPercentage = _validOperatorsMinimalPercentage;
    }

    function addOperator(address _addr, BlsOperations.G2Point calldata pubKey) external {
        requireOwner();
        require(
            !operators[_addr].exists,
            "osar11"
        ); // osar11 - operator exists
        operators[_addr] = Operator(true, pubKey);
        operatorsCount++;
    }

    function removeOperator(address _addr) external {
        requireOwner();
        require(
            operators[_addr].exists,
            "osrr11"
        ); // osar11 - operator does not exists
        delete(operators[_addr]);
        operatorsCount--;
    }
    
    function verify(
        Signature[] calldata _signatures,
        bytes calldata _message
    ) external view returns (bool) {
        require(
            _signatures.length >= operatorsCount * validOperatorsMinimalPercentage / 100,
            "osvy1"
        ); // osvy1 - signatures array length must be equal or more than allowed operators minimal count to verify message
        
        for (uint256 i = 0; i < _signatures.length; i++) {
            if(!operators[_signatures[i].operator].exists) {
                revert("osvy2"); // osvy2 - unknown operator
            }
        }

        BlsOperations.G1Point memory aggrPubKey;
        for(uint256 i = 0; i < _signatures.length; i++) {
            aggrPubKey = BlsOperations.addG2(aggrPubKey, operators[_signatures[i].operator].pubKey);
        }

        BlsOperations.G1Point memory mpoint = BlsOperations.messageToG1(_message);

        BlsOperations.G1Point memory aggrSignature;
        for(uint256 i = 0; i < _signatures.length; i++) {
            aggrSignature = BlsOperations.addG1(aggrSignature, _signatures[i].signature);
        }
        
        return BlsOperations.twoPointsPairing(BlsOperations.negate(aggrSignature), mpoint, BlsOperations.generatorG2(), aggrPubKey);
    }

    // Check if the sender is owner
    function requireOwner() internal view {
        require(
            msg.sender == ownerAddress,
            "osrr21"
        ); // osrr21 - only by owner
    }
}