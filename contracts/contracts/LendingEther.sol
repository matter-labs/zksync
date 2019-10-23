pragma solidity ^0.5.8;

import "./LendingToken.sol";
import "./ReentrancyGuard.sol";

contract LendingEther is LendingToken, ReentrancyGuard {
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

    function supply(uint256 _amount, address _to) external payable nonReentrant {
        supplyInternal(_amount, _to);
    }

    function transferIn(uint256 _amount) internal;

    function withdraw(uint256 _amount, address _to) external nonReentrant {
        withdrawInternal(_amount, _to);
    }

    function transferOut(uint256 _amount, address _to) internal {
        _to.transfer(_amount);
    }

    function requestBorrow(
        bytes32 _txHash,
        bytes _signature,
        uint256 _amount,
        address _borrower,
        address _receiver,
        uint32 _blockNumber
    ) external {
        requestBorrowInternal(
            _txHash,
            _signature,
            _amount,
            _borrower,
            _receiver,
            _blockNumber
        );
    }

    function getCurrentInterestRates() external pure nonReentrant returns (uint256 _borrowing, uint256 _supply) {
        return getCurrentInterestRatesInternal();
    }

    function fulfillOrder(
        uint32 _blockNumber,
        uint32 _orderId,
        uint256 _sendingAmount,
        address _lender
    ) external payable nonReentrant {
        fulfillOrderInternal(
            _blockNumber,
            _orderId,
            _sendingAmount,
            _lender
        );
    }

    function newVerifiedBlock(uint32 _blockNumber) external nonReentrant {
        newVerifiedBlockInternal(_blockNumber);
    }

    function repayBorrow(uint256 _amount) external payable nonReentrant {
        repayBorrowInternal(_amount);
    }
}
