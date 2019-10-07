pragma solidity ^0.5.8;

contract Multisig {

    struct G1Point {
        uint256 X;
        uint256 Y;
    }

    struct G2Point {
        uint256[2] X;
        uint256[2] Y;
    }

    function generatorG1() internal pure returns (G1Point memory) {
        return G1Point(1, 1);
    }

    function generatorG2() internal pure returns (G2Point memory) {
        return G2Point({
            x: [
                1,
                1
            ],
            y: [
                1,
                1
            ]
        });
    }

    function mulG1(
        G1Point memory _point,
        uint _scalar
    ) internal returns (G1Point memory output) {
        uint[3] memory input = [_point.x, _point.y, _scalar];

        assembly {
            if iszero(staticcall(sub(gas, 2000), 7, input, 0x80, output, 0x60)) {
                invalid()
            }
        }
    }

    function hashToG1(bytes memory _message) internal returns (G1Point memory point) {
        uint256 h = uint256(keccak256(_message));
        point = mulG1(generatorG1(), h);
    }

    function aggregate(
        G1Point[] calldata _sigs,
        G2Point[] calldata _pubKeys
    ) external returns (G1Point memory sig, G2Point memory pubKey) {
        require(_sigs.length != 0, "mae1"); // mae1 - signatures array length must be more than 0
        require(_pubKeys.length != 0, "mae2"); // mae2 - pubkeys array length must be more than 0
        require(_pubKeys.length == _sigs.length, "mae3"); // mae3 - signatures array length must be equal to pubkeys array length

        sig = _sigs[0];
        pubKey = _pubKeys[0];
        
        for (uint256 i = 1; i < _sigs.length; i++)
        {
            sig = concatG1(sig, _sigs[i]);
        } 

        for (uint256 i = 1; i < _pubKeys.length; i++)
        {
            pubKey = concatG2(pubKey, _pubKeys[i]);
        }
    }
    
    function verify(
        G1Point calldata _sig,
        G2Point calldata _pubKey,
        bytes calldata _message
    ) external returns (bool) {
        G1Point memory mpoint = hashToG1(_message);
        return pairing(negate(_sig), generatorG2(), mpoint, _pubKey);
    }
}