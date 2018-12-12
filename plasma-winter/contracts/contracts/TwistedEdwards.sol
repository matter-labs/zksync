pragma solidity ^0.4.24;

// This contract implements Twisted Edwards Curve arithmetics 
// for a SPECIFIC HARDCODED curve

// Since we don't use ABIv2, it's main purpose is to check that the point is
// in specific group

// If one wants external functionality - just wrap internal functions with
// Edwards Point represented as uint256[2]

// parameters: 
// a
// d
// prime field size 
// cofactor
// main group order


// TODO: should consider Montgommery ladder here
contract TwistedEdwards {
    EdwardsPoint public generator;

    struct EdwardsPoint {
        uint256 x;
        uint256 y;
    }

    constructor (
        uint256[2] memory _generator
    ) public {
        require(_generator[0] < getPrimeFieldSize(), "Generator X is not in the field");
        require(_generator[1] < getPrimeFieldSize(), "Generator Y is not in the field");
        // TODO: Check generator order
        generator = EdwardsPoint (_generator[0], _generator[1]);
    }

    function getA()
    internal
    pure 
    returns (uint256) {
        return 21888242871839275222246405745257275088548364400416034343698204186575808495616;
    }

    function getD()
    internal
    pure 
    returns (uint256) {
        return 12181644023421730124874158521699555681764249180949974110617291017600649128846;
    }

    function getCofactor()
    internal
    pure 
    returns (uint256) {
        return 8;
    }

    function getMainGroupOrder()
    internal
    pure 
    returns (uint256) {
        return 2736030358979909402780800718157159386076813972158567259200215660948447373041;
    }

    function getPrimeFieldSize()
    internal
    pure 
    returns (uint256) {
        return 21888242871839275222246405745257275088548364400416034343698204186575808495617;
    }

    function fieldNegate(uint256 _x)
    internal
    pure
    returns (uint256) {
        return getPrimeFieldSize() - _x;
    }

    function negate(EdwardsPoint memory P)
    internal
    pure
    returns (EdwardsPoint memory S)
    {
        S = EdwardsPoint(fieldNegate(P.x), P.y);
    }

    function fieldInv(uint256 x) 
    internal 
    pure returns (uint256) {
        uint256 p = getPrimeFieldSize();
        uint256 a = x;
        if (a == 0)
            return 0;
        if (a > p)
            a = a % p;
        int256 t1;
        int256 t2 = 1;
        uint256 r1 = p;
        uint256 r2 = a;
        uint256 q;
        while (r2 != 0) {
            q = r1 / r2;
            (t1, t2, r1, r2) = (t2, t1 - int256(q) * t2, r2, r1 - q * r2);
        }
        if (t1 < 0)
            return (p - uint256(-t1));
        return uint256(t1);
    }

    // Infinity point is encoded as (0, 1)
    function isInfinity(EdwardsPoint memory P)
    internal
    pure
    returns (bool)
    {  
        return P.x == 0 && P.y == 1;
    }

    // group addition law
    // x3 = (x1*y2+y1*x2)/(1+d*x1*x2*y1*y2)
    // y3 = (y1*y2-a*x1*x2)/(1-d*x1*x2*y1*y2)

    // IMPORTANT! Take no assumption about a == -1 for now
    function add(EdwardsPoint memory P, EdwardsPoint memory Q)
    internal
    pure
    returns (EdwardsPoint memory S)
    {
        uint256 p = getPrimeFieldSize();
        uint256 a = getA();
        uint256 d = getD();

        // precompute and save x1*y2. x2*y1
        uint256 x1y2 = mulmod(P.x, Q.y, p);
        uint256 x2y1 = mulmod(Q.x, P.y, p);
        // calculate x1*x2 and y1*y2 for shortness
        uint256 x1x2 = mulmod(P.x, Q.x, p);
        uint256 y1y2 = mulmod(P.y, Q.y, p);

        uint256 x3_t = addmod(x1y2, x2y1, p);
        uint256 x3_b = fieldInv(addmod(1, mulmod( mulmod(d, x1y2, p), x2y1, p), p) );

        // manual negations here
        uint256 y3_t = addmod(y1y2, p - mulmod(a, x1x2, p), p);
        uint256 y3_b = fieldInv(addmod(1, p - mulmod( mulmod(d, x1y2, p), x2y1, p), p) );
        
        S = EdwardsPoint(mulmod(x3_t, x3_b, p), mulmod(y3_t, y3_b, p));
    }

    // group doubling law
    // x3 = (x1*y1+y1*x1)/(1+d*x1*x1*y1*y1)
    // y3 = (y1*y1-a*x1*x1)/(1-d*x1*x1*y1*y1)

    // IMPORTANT! Take no assumption about a == -1 for now
    function double(EdwardsPoint memory P)
    internal
    pure
    returns (EdwardsPoint memory S)
    {
        uint256 p = getPrimeFieldSize();
        uint256 a = getA();
        uint256 d = getD();

        // precompute and save x1*y2. x2*y1
        uint256 xx = mulmod(P.x, P.x, p);
        uint256 yy = mulmod(P.y, P.y, p);
        uint256 xy = mulmod(P.x, P.y, p);

        uint256 x3_t = addmod(xy, xy, p);
        uint256 x3_b = fieldInv(addmod(1, mulmod( mulmod(d, xy, p), xy, p), p) );

        // manual negations here
        uint256 y3_t = addmod(yy, p - mulmod(a, xx, p), p);
        uint256 y3_b = fieldInv(addmod(1, p - mulmod( mulmod(d, xx, p), yy, p), p) );
        
        S = EdwardsPoint(mulmod(x3_t, x3_b, p), mulmod(y3_t, y3_b, p));
    }

    
    function multiplyByScalar(
        uint256 d, 
        EdwardsPoint memory P
    ) 
    internal 
    pure
    returns (EdwardsPoint memory S)
    {
        
        S = EdwardsPoint(0,1);
        if (d == 0) {
            return S;
        }

        EdwardsPoint memory base = EdwardsPoint(P.x, P.y);

        // double and add
        uint256 remaining = d;
        while (remaining != 0) {
            if ((remaining & 1) != 0) {
                S = add(S, base);
            }
            remaining = remaining >> 1;
            base = double(base);
        }

    }

    // Check that a * x^2 + y^2 = 1 + d * x^2 * y^2
    function isOnCurve(
        EdwardsPoint memory P
    )
    internal
    pure
    returns (bool)
    {
        uint256 p = getPrimeFieldSize();
        uint256 a = getA();
        uint256 d = getD();

        uint256 xx = mulmod(P.x, P.x, p);
        uint256 yy = mulmod(P.y, P.y, p);

        uint256 lhs = addmod(mulmod(a, xx,p), yy, p);
        uint256 rhs = addmod(1, mulmod(d, mulmod(xx, yy, p), p), p);

        return lhs == rhs;
    }

    function isInCorrectGroup(
        EdwardsPoint memory P
    ) 
    internal 
    pure
    returns (bool)
    {
        uint256 order = getMainGroupOrder();
        return isInfinity(multiplyByScalar(order, P));
    }

    function isCorrectGroup(
        uint256[2] memory point
    )
    public
    pure
    returns (bool)
    {
        EdwardsPoint memory P = EdwardsPoint(point[0], point[1]);
        return isInCorrectGroup(P);
    }

    function multiply(
        uint256 d,
        uint256[2] memory point
    )
    public
    pure
    returns (uint256[2] memory result)
    {
        EdwardsPoint memory P = EdwardsPoint(point[0], point[1]);
        EdwardsPoint memory S = multiplyByScalar(d, P);
        result[0] = S.x;
        result[1] = S.y;
    }

    function checkOnCurve(
        uint256[2] memory point
    )
    public
    pure
    returns (bool) {
        EdwardsPoint memory P = EdwardsPoint(point[0], point[1]);
        return isOnCurve(P);
    }

    // // Multiplication dP. P affine, wNAF: w=5
    // // Params: d, Px, Py
    // // Output: Jacobian Q
    // function _wnafMul(
    //     uint256 d, 
    //     EdwardsPoint memory P
    // ) 
    // internal 
    // pure 
    // returns (EdwardsPoint memory S)
    // {
    //     uint p = getPrimeFieldSize();
    //     if (d == 0) {
    //         return pointOfInfinity;
    //     }
    //     uint dwPtr; // points to array of NAF coefficients.
    //     uint i;

    //     // wNAF
    //     assembly
    //     {
    //         let dm := 0
    //         dwPtr := mload(0x40)
    //         mstore(0x40, add(dwPtr, 512)) // Should lower this.
    //     loop:
    //         jumpi(loop_end, iszero(d))
    //         jumpi(even, iszero(and(d, 1)))
    //         dm := mod(d, 32)
    //         mstore8(add(dwPtr, i), dm) // Don't store as signed - convert when reading.
    //         d := add(sub(d, dm), mul(gt(dm, 16), 32))
    //     even:
    //         d := div(d, 2)
    //         i := add(i, 1)
    //         jump(loop)
    //     loop_end:
    //     }

    //     // Pre calculation
    //     uint[3][8] memory PREC; // P, 3P, 5P, 7P, 9P, 11P, 13P, 15P
    //     PREC[0] = [P[0], P[1], 1];
    //     uint[3] memory X = _double(PREC[0]);
    //     PREC[1] = _addMixed(X, P);
    //     PREC[2] = _add(X, PREC[1]);
    //     PREC[3] = _add(X, PREC[2]);
    //     PREC[4] = _add(X, PREC[3]);
    //     PREC[5] = _add(X, PREC[4]);
    //     PREC[6] = _add(X, PREC[5]);
    //     PREC[7] = _add(X, PREC[6]);

    //     // Mult loop
    //     while(i > 0) {
    //         uint dj;
    //         uint pIdx;
    //         i--;
    //         assembly {
    //             dj := byte(0, mload(add(dwPtr, i)))
    //         }
    //         Q = _double(Q);
    //         if (dj > 16) {
    //             pIdx = (31 - dj) / 2; // These are the "negative ones", so invert y.
    //             Q = _add(Q, [PREC[pIdx][0], p - PREC[pIdx][1], PREC[pIdx][2] ]);
    //         }
    //         else if (dj > 0) {
    //             pIdx = (dj - 1) / 2;
    //             Q = _add(Q, [PREC[pIdx][0], PREC[pIdx][1], PREC[pIdx][2] ]);
    //         }
    //         if (Q[0] == pointOfInfinity[0] && Q[1] == pointOfInfinity[1] && Q[2] == pointOfInfinity[2]) {
    //             return Q;
    //         }
    //     }
    //     return Q;
    // }







}