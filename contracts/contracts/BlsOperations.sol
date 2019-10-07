pragma solidity ^0.5.8;

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
                0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2,
                0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed
            ],
            y: [
                0x90689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b,
                0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa
            ]
        });
    }

    function mulG1(
        G1Point memory _point,
        uint256 _scalar
    ) internal view returns (G1Point memory output) {
        uint256[3] memory input = [_point.x, _point.y, _scalar];

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

    function addG1(
        G1Point memory _point1,
        G1Point memory _point2
    ) internal view returns (G1Point memory output) {
        uint256[4] memory input;
        input[0] = _point1.x;
        input[1] = _point1.y;
        input[2] = _point2.x;
        input[3] = _point2.y;
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

    // TODO: - probably not working
    function addG2(
        G2Point memory _point1,
        G2Point memory _point2
    ) internal view returns (G2Point memory output) {
        uint256[8] memory input;
        input[0] = _point1.x[0];
        input[1] = _point1.x[1];
        input[2] = _point1.y[0];
        input[3] = _point1.y[1];
        input[4] = _point2.x[0];
        input[5] = _point2.x[1];
        input[6] = _point2.y[0];
        input[7] = _point2.y[1];
        assembly {
            if iszero(
                staticcall(
                    sub(gas, 2000),
                    6,
                    input,
                    0x150,
                    output,
                    0x60
                )
            ) {
                invalid()
            }
        }
    }

    function messageToG1(bytes memory _message) internal view returns (G1Point memory) {
        uint256 hash = uint256(keccak256(_message));
        return mulG1(generatorG1(), hash);
    }

    function negate(G1Point memory _point) internal pure returns (G1Point memory) {
        uint256 prime = 0x30644E72E131A029B85045B68181585D97816A916871CA8D3C208C16D87CFD47;
        if (_point.x == 0 && _point.y == 0) {
            return G1Point(0, 0);
        }
        return G1Point(_point.x, prime - (_point.y % prime));
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