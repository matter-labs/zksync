pragma solidity ^0.5.8;

import "./Exponential.sol";
import "./InterestRateModel.sol";

/**
  * @title The Compound Standard Interest Rate Model with pluggable constants
  * @author Compound
  * @notice See Section 2.4 of the Compound Whitepaper
  */
contract WhitePaperInterestRateModel is InterestRateModel, Exponential {
    /**
     * @notice Indicator that this is an InterestRateModel contract (for inspection)
     */
    bool public constant isInterestRateModel = true;

    /**
     * @notice The multiplier of utilization rate that gives the slope of the interest rate
     */
    uint public multiplier;

    /**
     * @notice The base interest rate which is the y-intercept when utilization rate is 0
     */
    uint public baseRate;

    /**
     * @notice The approximate number of blocks per year that is assumed by the interest rate model
     */
    uint public constant blocksPerYear = 2102400;

    constructor(uint baseRate_, uint multiplier_) public {
        baseRate = baseRate_;
        multiplier = multiplier_;
    }

    enum IRError {
        NO_ERROR,
        FAILED_TO_ADD_CASH_PLUS_BORROWS,
        FAILED_TO_GET_EXP,
        FAILED_TO_MUL_UTILIZATION_RATE,
        FAILED_TO_ADD_BASE_RATE
    }

    /*
     * @dev Calculates the utilization rate (borrows / (cash + borrows)) as an Exp
     */
    function getUtilizationRate(uint cash, uint borrows) pure internal returns (IRError, Exp memory) {
        if (borrows == 0) {
            // Utilization rate is zero when there's no borrows
            return (IRError.NO_ERROR, Exp({mantissa: 0}));
        }

        (MathError err0, uint cashPlusBorrows) = addUInt(cash, borrows);
        if (err0 != MathError.NO_ERROR) {
            return (IRError.FAILED_TO_ADD_CASH_PLUS_BORROWS, Exp({mantissa: 0}));
        }

        (MathError err1, Exp memory utilizationRate) = getExp(borrows, cashPlusBorrows);
        if (err1 != MathError.NO_ERROR) {
            return (IRError.FAILED_TO_GET_EXP, Exp({mantissa: 0}));
        }

        return (IRError.NO_ERROR, utilizationRate);
    }

    /*
     * @dev Calculates the utilization and borrow rates for use by getBorrowRate function
     */
    function getUtilizationAndAnnualBorrowRate(uint cash, uint borrows) view internal returns (IRError, Exp memory, Exp memory) {
        (IRError err0, Exp memory utilizationRate) = getUtilizationRate(cash, borrows);
        if (err0 != IRError.NO_ERROR) {
            return (err0, Exp({mantissa: 0}), Exp({mantissa: 0}));
        }

        // Borrow Rate is 5% + UtilizationRate * 45% (baseRate + UtilizationRate * multiplier);
        // 45% of utilizationRate, is `rate * 45 / 100`
        (MathError err1, Exp memory utilizationRateMuled) = mulScalar(utilizationRate, multiplier);
        // `mulScalar` only overflows when the product is >= 2^256.
        // utilizationRate is a real number on the interval [0,1], which means that
        // utilizationRate.mantissa is in the interval [0e18,1e18], which means that 45 times
        // that is in the interval [0e18,45e18]. That interval has no intersection with 2^256, and therefore
        // this can never overflow for the standard rates.
        if (err1 != MathError.NO_ERROR) {
            return (IRError.FAILED_TO_MUL_UTILIZATION_RATE, Exp({mantissa: 0}), Exp({mantissa: 0}));
        }

        (MathError err2, Exp memory utilizationRateScaled) = divScalar(utilizationRateMuled, mantissaOne);
        // 100 is a constant, and therefore cannot be zero, which is the only error case of divScalar.
        assert(err2 == MathError.NO_ERROR);

        // Add the 5% for (5% + 45% * Ua)
        (MathError err3, Exp memory annualBorrowRate) = addExp(utilizationRateScaled, Exp({mantissa: baseRate}));
        // `addExp` only fails when the addition of mantissas overflow.
        // As per above, utilizationRateMuled is capped at 45e18,
        // and utilizationRateScaled is capped at 4.5e17. mantissaFivePercent = 0.5e17, and thus the addition
        // is capped at 5e17, which is less than 2^256. This only applies to the standard rates
        if (err3 != MathError.NO_ERROR) {
            return (IRError.FAILED_TO_ADD_BASE_RATE, Exp({mantissa: 0}), Exp({mantissa: 0}));
        }

        return (IRError.NO_ERROR, utilizationRate, annualBorrowRate);
    }

    /**
      * @notice Gets the current borrow interest rate based on the given asset, total cash, total borrows
      *         and total reserves.
      * @dev The return value should be scaled by 1e18, thus a return value of
      *      `(true, 1000000000000)` implies an interest rate of 0.000001 or 0.0001% *per block*.
      * @param cash The total cash of the underlying asset in the CToken
      * @param borrows The total borrows of the underlying asset in the CToken
      * @param _reserves The total reserves of the underlying asset in the CToken
      * @return Success or failure and the borrow interest rate per block scaled by 10e18
      */
    function getBorrowRate(uint cash, uint borrows, uint _reserves) public view returns (uint, uint) {
        _reserves; // pragma ignore unused argument

        (IRError err0, Exp memory _utilizationRate, Exp memory annualBorrowRate) = getUtilizationAndAnnualBorrowRate(cash, borrows);
        if (err0 != IRError.NO_ERROR) {
            return (uint(err0), 0);
        }

        // And then divide down by blocks per year.
        (MathError err1, Exp memory borrowRate) = divScalar(annualBorrowRate, blocksPerYear); // basis points * blocks per year
        // divScalar only fails when divisor is zero. This is clearly not the case.
        assert(err1 == MathError.NO_ERROR);

        _utilizationRate; // pragma ignore unused variable

        // Note: mantissa is the rate scaled 1e18, which matches the expected result
        return (uint(IRError.NO_ERROR), borrowRate.mantissa);
    }
}
