pragma solidity ^0.5.8;

import "./BlsOperations.sol";

contract Operators {

    address internal ownerAddress;

    struct Operator {
        bool exists;
        BlsOperations.G2Point pubKey;
    }

    uint256 public minSigsPercentage;

    uint256 public operatorsCount = 0;
    mapping (address => Operator) private operators;

    constructor(address _ownerAddress, uint256 _minSigsPercentage) public {
        require(
            _minSigsPercentage <= 100 && _minSigsPercentage > 0,
            "oscr11"
        ); // osar11 - we need operators percentage be between 0% and 100%
        ownerAddress = _ownerAddress;
        minSigsPercentage = _minSigsPercentage;
    }

    function isOperator(address _addr) external view returns (bool) {
        return operators[_addr].exists;
    }

    function addOperator(
        address _addr,
        uint256 _pbkxx,
        uint256 _pbkxy,
        uint256 _pbkyx,
        uint256 _pbkyy
    ) external {
        requireOwner();
        require(
            !operators[_addr].exists,
            "osar11"
        ); // osar11 - operator exists
        operators[_addr] = Operator(
            true,
            BlsOperations.G2Point({
                x: [
                    _pbkxx,
                    _pbkxy
                ],
                y: [
                    _pbkyx,
                    _pbkyy
                ]
            })
        );
        operatorsCount++;
    }

    function changeMinSigsPercentage(uint256 _newMinSigsPercentage) external {
        requireOwner();
        require(
            _newMinSigsPercentage <= 100 && _newMinSigsPercentage > 0,
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
        address[] calldata _signers,
        uint256[] calldata _signatures,
        bytes calldata _message
    ) external view returns (bool) {
        require(
            _signers.length >= operatorsCount * minSigsPercentage / 100,
            "osvy1"
        ); // osvy1 - signers array length must be equal or more than allowed operators minimal count to verify message
        require(
            _signatures.length == 2 * _signers.length,
            "osvy2"
        ); // osvy2 - signatures array length must be equal to 2 * signers array length (signature is G1 point that consists of uint256 x and y)

        BlsOperations.G2Point memory aggrPubKey;
        BlsOperations.G1Point memory aggrSignature;
        uint256 k = 0;
        for (uint256 i = 0; i < _signers.length; i++) {
            if(!operators[_signers[i]].exists) {
                revert("osvy3"); // osvy3 - unknown operator
            }

            BlsOperations.G1Point memory sign = BlsOperations.G1Point({
                x: _signatures[k],
                y: _signatures[k+1]
            });

            aggrPubKey = BlsOperations.addG2(aggrPubKey, operators[_signers[i]].pubKey);
            aggrSignature = BlsOperations.addG1(aggrSignature, sign);

            k += 2;
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