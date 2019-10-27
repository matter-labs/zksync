pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./LendingToken.sol";
import "./ReentrancyGuard.sol";

/// @title Lending ERC20 Contract
/// @notice Interface for LendingToken contract
/// @author Matter Labs
contract LendingErc20 is LendingToken, ReentrancyGuard {

    /// @notice Construct a new ERC20 lending
    /// @dev Constructs a new token lending via LendingToken constructor
    /// @param _tokenAddress The address of the specified ERC20 token
    /// @param _governanceAddress The address of Governance contract
    /// @param _franklinAddress The address of Franklin contract
    /// @param _verifierAddress The address of Verifier contract
    /// @param _owner The address of this contracts owner (Matter Labs)
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

    /// @notice Fallback function
    function() external payable {
        fallbackInternal();
    }

    /// @notice Transfers specified amount of ERC20 token into lending
    /// @dev Sender is specified in msg.sender
    /// @param _amount Amount in ERC20 tokens
    function transferIn(uint256 _amount) internal {
        require(
            IERC20(token.tokenAddress).transferFrom(msg.sender, address(this), _amount),
            "lctn11"
        ); // lctn11 - token transfer in failed
    }

    /// @notice Transfers specified amount of ERC20 token to external address
    /// @param _amount Amount in ERC20 tokens
    /// @param _to Receiver
    function transferOut(uint256 _amount, address _to) internal {
        require(
            IERC20(token.tokenAddress).transfer(_to, _amount),
            "lctt11"
        ); // lctt11 - token transfer out failed
    }

    /// @notice Supplies specified amount of ERC20 token from lender
    /// @param _amount Amount in ERC20 tokens
    /// @param _lender Lender account address
    function supply(uint256 _amount, address _lender) external nonReentrant {
        supplyInternal(_amount, _lender);
    }
    
    /// @notice Lender can request withdraw of her funds
    /// @param _amount Amount in ERC20 tokens
    /// @param _to Receiver
    function requestWithdraw(uint256 _amount, address _to) external nonReentrant {
        requestWithdrawInternal(_amount, _to);
    }

    /// @notice User can request borrow, providing his withdraw operation in Franklin block
    /// @param _blockNumber The number of committed block with withdraw operation
    /// @param _requestNumber Withdraw operation creates request. The user is needed to provide this number
    /// @param _amount The borrow amount
    /// @param _borrower Borrower address
    /// @param _receiver Receiver address
    /// @param _signature Borrow request signature
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

    /// @notice Calculates current interest rates
    /// @return Borrowing and supply interest rates
    function getCurrentInterestRates() external pure nonReentrant returns (uint256 _borrowing, uint256 _supply) {
        return getCurrentInterestRatesInternal();
    }

    /// @notice Lender can fulfill deffered borrow order
    /// @param _blockNumber The number of committed block with withdraw operation
    /// @param _orderId Specified order id
    /// @param _amount The borrow amount of ERC20 tokens
    /// @param _lender Lender address
    function fulfillDefferedBorrowOrder(
        uint32 _blockNumber,
        uint32 _orderId,
        uint256 _sendingAmount,
        address _lender
    ) external nonReentrant {
        fulfillDefferedBorrowOrderInternal(
            _blockNumber,
            _orderId,
            _sendingAmount,
            _lender
        );
    }

    /// @notice Called by Franklin contract when a new block is verified. Removes this blocks' orders and consummates fees
    /// @param _blockNumber The number of committed block with withdraw operation
    function newVerifiedBlock(uint32 _blockNumber) external nonReentrant {
        newVerifiedBlockInternal(_blockNumber);
    }

    /// @notice Called by Franklin contract to provide borrowed fees from withdraw operation
    /// @dev Amount of ERC20 tokens is provided to repayBorrowInternal as msg.value
    /// @param _amount The amount specified in withdraw operation
    function repayBorrow(uint256 _amount) external nonReentrant {
        repayBorrowInternal(_amount);
    }
}
