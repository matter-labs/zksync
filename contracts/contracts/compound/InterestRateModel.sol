pragma solidity ^0.5.8;

/**
  * @title The Compound InterestRateModel Interface
  * @author Compound
  * @notice Any interest rate model should derive from this contract.
  * @dev These functions are specifically not marked `pure` as implementations of this
  *      contract may read from storage variables.
  */
interface InterestRateModel {
    /**
      * @notice Gets the current borrow interest rate based on the given asset, total cash, total borrows
      *         and total reserves.
      * @dev The return value should be scaled by 1e18, thus a return value of
      *      `(true, 1000000000000)` implies an interest rate of 0.000001 or 0.0001% *per block*.
      * @param cash The total cash of the underlying asset in the CToken
      * @param borrows The total borrows of the underlying asset in the CToken
      * @param reserves The total reserves of the underlying asset in the CToken
      * @return Success or failure and the borrow interest rate per block scaled by 10e18
      */
    function getBorrowRate(uint cash, uint borrows, uint reserves) external view returns (uint, uint);

    /**
      * @notice Marker function used for light validation when updating the interest rate model of a market
      * @dev Marker function used for light validation when updating the interest rate model of a market. Implementations should simply return true.
      * @return Success or failure
      */
    function isInterestRateModel() external view returns (bool);
}