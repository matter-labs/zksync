pragma solidity ^0.5.8;
pragma experimental ABIEncoderV2;

import "./BlsOperations.sol";

contract Operators {

    address internal ownerAddress;

    struct Operator {
        bool exists;
        BlsOperations.G2Point pubKey;
    }

    struct Signature {
        address operator;
        BlsOperations.G1Point signature;
    }

    uint256 public minSigsPercentage;

    uint256 public operatorsCount = 0;
    mapping (address => Operator) public operators;

    constructor(address _ownerAddress, uint256 _minSigsPercentage) public {
        require(
            _minSigsPercentage <= 1 && _minSigsPercentage > 0,
            "oscr11"
        ); // osar11 - we need operators percentage be between 0% and 100%
        ownerAddress = _ownerAddress;
        minSigsPercentage = _minSigsPercentage;
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

    function changeMinSigsPercentage(uint256 _newMinSigsPercentage) external {
        requireOwner();
        require(
            _newMinSigsPercentage <= 1 && _newMinSigsPercentage > 0,
            "osce11"
        ); // osce11 - we need operators percentage be between 0% and 100%
        minSigsPercentage = _newMinSigsPercentage;
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
            _signatures.length >= operatorsCount * minSigsPercentage,
            "osvy1"
        ); // osvy1 - signatures array length must be equal or more than allowed operators minimal count to verify message
        
        BlsOperations.G2Point memory aggrPubKey;
        BlsOperations.G1Point memory aggrSignature;
        for (uint256 i = 0; i < _signatures.length; i++) {
            if(!operators[_signatures[i].operator].exists) {
                revert("osvy2"); // osvy2 - unknown operator
            }
            aggrPubKey = BlsOperations.addG2(aggrPubKey, operators[_signatures[i].operator].pubKey);
            aggrSignature = BlsOperations.addG1(aggrSignature, _signatures[i].signature);
        }

        BlsOperations.G1Point memory mpoint = BlsOperations.messageToG1(_message);

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