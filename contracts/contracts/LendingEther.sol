pragma solidity ^0.5.8;

import "./LendingToken.sol";

contract LendingEther is LendingToken {
    constructor(
        address _governanceAddress,
        address _franklinAddress,
        address _verifierAddress,
        address _owner
    ) public
    LendingToken(
        address(0),
        _governanceAddress,
        _franklinAddress,
        _verifierAddress,
        _owner
    ) {}

    function supply(uint256 _amount, address _lender) external {
        supplyInternal(_amount, _lender);
    }

    function transferIn(uint256 _amount, address _lender) internal;

    function withdraw(uint256 _amount) external {
        withdrawInternal(_amount);
    }

    function transferOut(uint256 _amount, address _lender) internal;

    function requestBorrow(
        bytes32 _txHash,
        bytes _signature,
        uint256 _amount,
        address _borrower,
        address _receiver,
        uint32 _blockNumber
    ) internal {
        requestBorrowInternal(
            _txHash,
            _signature,
            _amount,
            _borrower,
            _receiver,
            _blockNumber
        );
    }

    function getCurrentInterestRates() external pure returns (uint256 _borrowing, uint256 _supply) {
        return getCurrentInterestRatesInternal();
    }

    function fulfillOrder(
        uint32 _blockNumber,
        uint32 _orderId,
        uint256 _sendingAmount,
        address _lender
    ) external {
        fulfillOrderInternal(
            _blockNumber,
            _orderId,
            _sendingAmount,
            _lender
        );
    }

    function newVerifiedBlock(uint32 _blockNumber) external {
        newVerifiedBlockInternal(_blockNumber);
    }
}
