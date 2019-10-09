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

    function getOperatorPubkey(address _addr) external view returns (uint256, uint256, uint256, uint256) {
        return (operators[_addr].pubKey.x[0], operators[_addr].pubKey.x[1], operators[_addr].pubKey.y[0], operators[_addr].pubKey.y[1]);
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

    function aggregateSignersPubKeys(address[] calldata _signers) external view returns (uint256, uint256, uint256, uint256) {
        require(
            _signers.length >= operatorsCount * minSigsPercentage / 100,
            "osas1"
        ); // osas1 - signers array length must be equal or more than allowed operators minimal count to verify message
        BlsOperations.G2Point memory aggrPubKey;
        if (_signers.length == 1) {
            aggrPubKey = operators[_signers[0]].pubKey;
        } else {
            for (uint256 i = 0; i < _signers.length; i++) {
                if(!operators[_signers[i]].exists) {
                    revert("osas2"); // osas2 - unknown operator
                }
                aggrPubKey = BlsOperations.addG2(aggrPubKey, operators[_signers[i]].pubKey);
            }
        }
        return (aggrPubKey.x[0], aggrPubKey.x[1], aggrPubKey.y[0], aggrPubKey.y[1]);
    }

    function aggregateSignatures(uint256[] calldata _signatures, uint256 _signersCount) external view returns (uint256, uint256) {
        require(
            _signatures.length == 2 * _signersCount,
            "osas3"
        ); // osas3 - signatures array length must be equal to 2 * signers array length (signature is G1 point that consists of uint256 x and y)
        BlsOperations.G1Point memory aggrSignature;
        if (_signatures.length == 2) {
            aggrSignature = BlsOperations.G1Point({
                x: _signatures[0],
                y: _signatures[1]
            });
        } else {
            for (uint256 i = 0; i < _signatures.length; i += 2) {
                BlsOperations.G1Point memory sign = BlsOperations.G1Point({
                    x: _signatures[i],
                    y: _signatures[i+1]
                });
                aggrSignature = BlsOperations.addG1(aggrSignature, sign);
            }
        }
        return (aggrSignature.x, aggrSignature.y);
    }

    function verify(
        uint256 _signatureX,
        uint256 _signatureY,
        uint256 _pubkeyXX,
        uint256 _pubkeyXY,
        uint256 _pubkeyYX,
        uint256 _pubkeyYY,
        bytes calldata _message
    ) external view returns (bool) {
        BlsOperations.G1Point memory mpoint = BlsOperations.messageToG1(_message);
        BlsOperations.G1Point memory signature = BlsOperations.G1Point({
            x: _signatureX,
            y: _signatureY
        });
        BlsOperations.G2Point memory pubkey = BlsOperations.G2Point({
            x: [
                _pubkeyXX,
                _pubkeyXY
            ],
            y: [
                _pubkeyYX,
                _pubkeyYY
            ]
        });
        return BlsOperations.twoPointsPairing(BlsOperations.negate(signature), mpoint, BlsOperations.generatorG2(), pubkey);
    }

    // Check if the sender is owner
    function requireOwner() internal view {
        require(
            msg.sender == ownerAddress,
            "osrr21"
        ); // osrr21 - only by owner
    }
}