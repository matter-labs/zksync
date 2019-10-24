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

    // Source: https://github.com/matter-labs/RSAAccumulator/blob/master/contracts/RSAAccumulator.sol#L317
    // this assumes that exponent in never larger than 256 bits
    function modularExpU512(
        uint256[2] memory base,
        uint256 e,
        uint256[2] memory m)
    public view returns (uint256[2] output) {
        uint256 modulusLength = 64;
        uint256 memoryPointer = 0;
        uint256 dataLength = 0;
        assembly {
            // define pointer
            memoryPointer := mload(0x40)
            // store data assembly-favouring ways
            mstore(memoryPointer, modulusLength)    // Length of Base
            mstore(add(memoryPointer, 0x20), 0x20)  // Length of Exponent
            mstore(add(memoryPointer, 0x40), modulusLength)  // Length of Modulus
        }
        dataLength = 0x60;
        // now properly pack bases, etc
        uint256 limb = 0;
        for (uint256 i = 0; i < 2; i++) {
            limb = base[i];
            assembly {
                mstore(add(memoryPointer, dataLength), limb)  // cycle over base
            }
            dataLength += 0x20;
        }

        assembly {
            mstore(add(memoryPointer, dataLength), e)     // Put exponent
        }
        dataLength += 0x20;

        for (i = 0; i < 2; i++) {
            limb = m[i];
            assembly {
                mstore(add(memoryPointer, dataLength), limb)  // cycle over base
            }
            dataLength += 0x20;
        }
        // do the call
        assembly {
            let success := staticcall(sub(gas, 2000), 0x05, memoryPointer, dataLength, memoryPointer, modulusLength) // here we overwrite!
            // gas fiddling
            switch success case 0 {
                revert(0, 0)
            }
        }
        dataLength = 0;
        limb = 0;
        for (i = 0; i < 2; i++) {
            assembly {
                limb := mload(add(memoryPointer, dataLength))
            }
            dataLength += 0x20;
            output[i] = limb;
        }
        return output;
    }

    function hash512(bytes memory _input) internal view returns (bytes32, bytes32) {
        bytes memory zero = [0];
        bytes memory one = [1];
        return 
        (
            keccak256(
                Bytes.concat(_input, zero)
            ),
            keccak256(
                Bytes.concat(_input, one)
            )
        );
    }

    function messageToG1(bytes memory _message) internal view returns (G1Point memory) {
        bytes memory h = Bytes.toBytesFromBytes32(keccak256(_message), 32);
        
        uint256[2] memory h1_256;
        (bytes32 h1_256_0, bytes32 h1_256_1) = hash512(
            Bytes.concat(
                h,
                Bytes.toBytesFromUInt256(generatorG1().x)
            )
        );
        h1_256[0] = uint256(h1_256_0);
        h1_256[1] = uint256(h1_256_1);
        uint256[2] memory q1;
        q1[0] = 0;
        q1[1] = BN256G2.FIELD_MODULUS;
        (uint256 t00, uint256 t01) = modularExpU512(h1_256, 1, q1);

        uint256[2] memory h2_256;
        (bytes32 h2_256_0, bytes32 h2_256_1) = hash512(
            Bytes.concat(
                h,
                Bytes.toBytesFromUInt256(generatorG1().y)
            )
        );
        h2_256[0] = uint256(h2_256_0);
        h2_256[1] = uint256(h2_256_1);
        uint256[2] memory q2;
        q2[0] = 0;
        q2[1] = BN256G2.FIELD_MODULUS;
        (uint256 t10, uint256 t11) = modularExpU512(h2_256, 1, q2);

        // p <- swEncode(t0) * swEncode(t1)
        // return p;
    }

    // function messageToG2(bytes memory _message) internal view returns (G2Point memory) {
    //     uint256 hash = uint256(keccak256(_message));
    //     G2Point memory g2 = generatorG2();
    //     uint256 x1;
    //     uint256 x2;
    //     uint256 y1;
    //     uint256 y2;
    //     (x1, x2, y1, y2) = BN256G2.ECTwistMul(
    //         hash,
    //         g2.x[1],
    //         g2.x[0],
    //         g2.y[1],
    //         g2.y[0]
    //     );
    //     return G2Point({
    //         x: [
    //             x2,
    //             x1
    //         ],
    //         y: [
    //             y2,
    //             y1
    //         ]
    //     });
    // }

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