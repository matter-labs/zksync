pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./LendingToken.sol";
import "./ReentrancyGuard.sol";

contract LendingErc20 is LendingToken, ReentrancyGuard {
    constructor(
        address _tokenAddress,
        address _governanceAddress,
        address _franklinAddress,
        address _verifierAddress,
        address _owner
    ) public
    LendingToken(
        _tokenAddress,
        _governanceAddress,
        _franklinAddress,
        _verifierAddress,
        _owner
    ) {}

    function() external payable {
        fallbackInternal();
    }

    function supply(uint256 _amount, address _to) external nonReentrant {
        supplyInternal(_amount, _to);
    }

    function transferIn(uint256 _amount) internal {
        require(
            IERC20(token.tokenAddress).transferFrom(msg.sender, address(this), _amount),
            "lctn11"
        ); // lctn11 - token transfer in failed
    }

    function requestWithdraw(uint256 _amount, address _to) external nonReentrant {
        requestWithdrawInternal(_amount, _to);
    }

    function transferOut(uint256 _amount, address _to) internal {
        require(
            IERC20(token.tokenAddress).transfer(_to, _amount),
            "lctt11"
        ); // lctt11 - token transfer out failed
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
    ) external nonReentrant {
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

    function repayBorrow(uint256 _amount) external nonReentrant {
        repayBorrowInternal(_amount);
    }
}
