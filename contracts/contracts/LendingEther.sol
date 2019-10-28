pragma solidity ^0.5.8;

import "./LendingToken.sol";
import "./ReentrancyGuard.sol";

/// @title Lending Ether Contract
/// @notice Interface for LendingToken contract
/// @author Matter Labs
contract LendingEther is LendingToken, ReentrancyGuard {

    /// @notice Construct a new Ether lending
    /// @dev Constructs a new token lending via LendingToken constructor, providing address(0) as token address
    /// @param _governanceAddress The address of Governance contract
    /// @param _franklinAddress The address of Franklin contract
    /// @param _verifierAddress The address of Verifier contract
    /// @param _owner The address of this contracts owner (Matter Labs)
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

    /// @notice Fallback function
    function() external payable {
        fallbackInternal();
    }

    /// @notice Unused for Ether
    function transferIn(uint256 _amount) internal;

    /// @notice Transfers specified amount of Ether to external address
    /// @param _amount Amount in Ether
    /// @param _to Receiver
    function transferOut(uint256 _amount, address _to) internal {
        _to.transfer(_amount);
    }

    /// @notice Supplies specified amount of ether from lender
    /// @dev Calls supplyInternal with specified msg.value as amount
    /// @param _lender Lender account address
    function supply(address _lender) external payable nonReentrant {
        supplyInternal(msg.value, _lender);
    }

    /// @notice Lender can request withdraw of her funds
    /// @param _amount Amount in Ether
    /// @param _to Receiver
    function requestWithdraw(uint256 _amount, address _to) external nonReentrant {
        requestWithdrawInternal(_amount, _to);
    }

    /// @notice User can request borrow, providing his withdraw operation in Franklin block
    /// @param _onchainOpNumber Franklin onchain operation number
    /// @param _amount The borrow amount
    /// @param _borrower Borrower address
    /// @param _receiver Receiver address
    /// @param _signature Borrow request signature
    function requestBorrow(
        uint64 _onchainOpNumber,
        uint256 _amount,
        uint24 _borrower,
        address _receiver,
        bytes _signature
    ) external {
        requestBorrowInternal(
            _blockNumber,
            _requestNumber,
            _amount,
            _borrower,
            _receiver,
            _signature
        );
    }

    /// @notice Calculates current interest rates
    /// @return Borrowing and supply interest rates
    function getCurrentInterestRates() external pure nonReentrant returns (uint256 _borrowing, uint256 _supply) {
        return getCurrentInterestRatesInternal();
    }

    /// @notice Lender can fulfill deffered borrow order
    /// @dev Amount of Ether is provided to fulfillDefferedBorrowOrderInternal as msg.value
    /// @param _blockNumber The number of committed block with withdraw operation
    /// @param _orderId Specified order id
    /// @param _lender Lender address
    function fulfillDefferedBorrowOrder(
        uint32 _blockNumber,
        uint32 _orderId,
        address _lender
    ) external payable nonReentrant {
        fulfillDefferedBorrowOrderInternal(
            _blockNumber,
            _orderId,
            msg.value,
            _lender
        );
    }

    /// @notice Called by Franklin contract when a new block is verified. Removes this blocks' orders and consummates fees
    /// @param _blockNumber The number of committed block with withdraw operation
    function newVerifiedBlock(uint32 _blockNumber) external nonReentrant {
        newVerifiedBlockInternal(_blockNumber);
    }

    /// @notice Called by Franklin contract to provide borrowed fees from withdraw operation
    /// @dev Amount of Ether is provided to repayBorrowInternal as msg.value
    /// @param _amount The amount specified in withdraw operation
    function repayBorrow() external payable nonReentrant {
        repayBorrowInternal(msg.value);
    }
}
