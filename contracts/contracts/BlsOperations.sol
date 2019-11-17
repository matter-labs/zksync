pragma solidity ^0.5.8;

import "./BN256G2.sol";
import "./Bytes.sol";

library BlsOperations {
    struct G1Point {
        uint256 x;
        uint256 y;
    }

    struct G2Point {
        uint256[2] x;
        uint256[2] y;
    }

    function generatorG2() internal pure returns (G2Point memory) {
        return G2Point({
            x: [
                11559732032986387107991004021392285783925812861821192530917403151452391805634,
                10857046999023057135944570762232829481370756359578518086990519993285655852781
            ],
            y: [
                4082367875863433681332203403145435568316851327593401208105741076214120093531,
                8495653923123431417604973247489272438418190587263600148770280649306958101930
            ]
        });
    }

    function addG2(
        G2Point memory _point1,
        G2Point memory _point2
    ) internal view returns (G2Point memory) {
        (
            uint256 pt3xx,
            uint256 pt3xy,
            uint256 pt3yx,
            uint256 pt3yy
        ) = BN256G2.ECTwistAdd(
            _point1.x[0],
            _point1.x[1],
            _point1.y[0],
            _point1.y[1],
            _point2.x[0],
            _point2.x[1],
            _point2.y[0],
            _point2.y[1]
        );
        return G2Point ({
            x: [
                pt3xx,
                pt3xy
            ],
            y: [
                pt3yx,
                pt3yy
            ]
        });
    }

    function messageHashToG1(uint256 _messageHash) internal view returns (G1Point memory) {
        uint256 beta = 0;
        uint256 y = 0;
        uint256 x = _messageHash % BN256G2.fieldModulus();
        while( true ) {
            (beta, y) = findYforX(x);
            if(beta == mulmod(y, y, BN256G2.fieldModulus())) {
                return G1Point(x, y);
            }
            x = addmod(x, 1, BN256G2.fieldModulus());
        }
    }

    function findYforX(uint256 x) internal view returns (uint256, uint256) {
        // beta = (x^3 + b) % p
        uint256 beta = addmod(mulmod(mulmod(x, x, BN256G2.fieldModulus()), x, BN256G2.fieldModulus()), 3, BN256G2.fieldModulus());
        uint256 y = modPow(beta, BN256G2.curveA(), BN256G2.fieldModulus());
        return (beta, y);
    }

    function modPow(uint256 base, uint256 exponent, uint256 modulus) internal view returns (uint256) {
        uint256[1] memory result;
        assembly {
            let input := mload(0x40)
            mstore(input, 0x20)
            mstore(add(input, 0x20), 0x20)
            mstore(add(input, 0x40), 0x20)
            mstore(add(input, 0x60), base)
            mstore(add(input, 0x80), exponent)
            mstore(add(input, 0xa0), modulus)
            let value := mload(0xc0)
            if iszero(
                staticcall(
                    sub(gas, 2000),
                    5,
                    input,
                    0xc0,
                    result,
                    0x20
                )
            ) {
                invalid()
            }
        }
        return result[0];
    }

    function negate(uint256 value) internal pure returns (uint256) {
        uint256 field_modulus = BN256G2.fieldModulus();
        return field_modulus - (value % field_modulus);
    }

    function negate(G2Point memory _point) internal pure returns (G2Point memory) {
        uint256 zero = 0;
        if (_point.x[0] == 0 && _point.x[1] == 0 && _point.y[0] == 0 && _point.y[1] == 0) {
            return G2Point({
                x: [
                    zero,
                    zero
                ],
                y: [
                    zero,
                    zero
                ]
            });
        }
        return G2Point(
            [
                _point.x[0],
                _point.x[1]],
            [
                negate(_point.y[0]),
                negate(_point.y[1])
            ]
        );
    }

    function pairing(
        G1Point memory _g1point1,
        G2Point memory _g2point1,
        G1Point memory _g1point2,
        G2Point memory _g2point2
    ) internal view returns (bool) {
        uint256[12] memory input = [
            _g1point1.x, _g1point1.y,
            _g2point1.x[0], _g2point1.x[1], _g2point1.y[0], _g2point1.y[1],
            _g1point2.x, _g1point2.y,
            _g2point2.x[0], _g2point2.x[1], _g2point2.y[0], _g2point2.y[1]
        ];
        uint[1] memory output;
        assembly {
            if iszero(
                staticcall(
                    sub(gas, 2000),
                    8,
                    input,
                    0x180,
                    output,
                    0x20
                )
            ) {
                invalid()
            }
        }
        return output[0] != 0;
    }
}