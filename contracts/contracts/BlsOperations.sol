pragma solidity ^0.5.8;

import "./BN256G2.sol";

library BlsOperations {
    struct G1Point {
        uint256 x;
        uint256 y;
    }

    struct G2Point {
        uint256[2] x;
        uint256[2] y;
    }

    function generatorG1() internal pure returns (G1Point memory) {
        return G1Point({
            x: 1,
            y: 2
        });
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

    function mulG1(
        G1Point memory _point,
        uint256 _scalar
    ) internal view returns (G1Point memory output) {
        uint256[3] memory input = [
            _point.x,
            _point.y,
            _scalar
        ];
        assembly {
            if iszero(
                staticcall(
                    sub(gas, 2000),
                    7,
                    input,
                    0x80,
                    output,
                    0x60
                )
            ) {
                invalid()
            }
        }
    }

    function mulG2(
        G2Point memory _point,
        uint256 _scalar
    ) internal view returns (G2Point memory) {
        (
            uint256 pt2xx,
            uint256 pt2xy,
            uint256 pt2yx,
            uint256 pt2yy
        ) = BN256G2.ECTwistMul(
            _scalar,
            _point.x[0],
            _point.x[1],
            _point.y[0],
            _point.y[1]
        );
        return G2Point ({
            x: [
                pt2xx,
                pt2xy
            ],
            y: [
                pt2yx,
                pt2yy
            ]
        });
    }

    function addG1(
        G1Point memory _point1,
        G1Point memory _point2
    ) internal view returns (G1Point memory output) {
        uint256[4] memory input = [
            _point1.x,
            _point1.y,
            _point2.x,
            _point2.y
        ];
        assembly {
            if iszero(
                staticcall(
                    sub(gas, 2000),
                    6,
                    input,
                    0xc0,
                    output,
                    0x60
                )
            ) {
                invalid()
            }
        }
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

    function messageToG1(bytes memory _message) internal view returns (G1Point memory) {
        uint256 hash = uint256(keccak256(_message));
        return mulG1(generatorG1(), hash);
    }

    function messageToG2(bytes memory _message) internal view returns (G2Point memory) {
        uint256 hash = uint256(keccak256(_message));
        G2Point memory g2 = generatorG2();
        uint256 x1;
        uint256 x2;
        uint256 y1;
        uint256 y2;
        (x1, x2, y1, y2) = BN256G2.ECTwistMul(
            hash,
            g2.x[1],
            g2.x[0],
            g2.y[1],
            g2.y[0]
        );
        return G2Point({
            x: [
                x2,
                x1
            ],
            y: [
                y2,
                y1
            ]
        });
    }

    function negate(G1Point memory _point) internal pure returns (G1Point memory) {
        uint256 field_modulus = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
        if (_point.x == 0 && _point.y == 0) {
            return G1Point(0, 0);
        }
        return G1Point(_point.x, field_modulus - (_point.y % field_modulus));
    }

    function pairing(
        G1Point[] memory _g1points,
        G2Point[] memory _g2points
    ) internal view returns (bool) {
        require(
            _g1points.length == _g2points.length,
            "mvy1"
        ); // bspg1 - G1 and G2 points counts must be equal
        
        uint256 points = _g1points.length;
        uint256 inputSize = 6 * points;
        uint[] memory input = new uint256[](inputSize);

        for (uint i = 0; i < points; i++) {
            input[i * 6 + 0] = _g1points[i].x;
            input[i * 6 + 1] = _g1points[i].y;
            input[i * 6 + 2] = _g2points[i].x[0];
            input[i * 6 + 3] = _g2points[i].x[1];
            input[i * 6 + 4] = _g2points[i].y[0];
            input[i * 6 + 5] = _g2points[i].y[1];
        }

        uint256[1] memory output;
        assembly {
            if iszero(
                staticcall(
                    sub(gas, 2000),
                    8,
                    add(input, 0x20),
                    mul(inputSize, 0x20),
                    output,
                    0x20
                )
            ) {
                invalid()
            }
        }
        return output[0] == 1;
    }

    function twoPointsPairing(
        G1Point memory _g1point1,
        G1Point memory _g1point2,
        G2Point memory _g2point1,
        G2Point memory _g2point2
    ) internal view returns (bool) {
        G1Point[] memory g1points = new G1Point[](2);
        G2Point[] memory g2points = new G2Point[](2);
        g1points[0] = _g1point1;
        g1points[1] = _g1point2;
        g2points[0] = _g2point1;
        g2points[1] = _g2point2;
        return pairing(g1points, g2points);
    }
}