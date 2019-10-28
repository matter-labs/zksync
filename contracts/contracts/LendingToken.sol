pragma solidity ^0.5.8;

import "./Governance.sol";
import "./Franklin.sol";
import "./Verifier.sol";
import "./SafeMath.sol";

/// @title Lending Token Contract
/// @notice Inner logic for LendingEther and LendingErc20 token contracts
/// @author Matter Labs
contract LendingToken {
    
    using SafeMath for uint256;

    /// @notice Multiplier is used in calculation of borrowing interest rate
    uint256 constant MULTIPLIER = 45;

    /// @notice Base rate is used in calculation of borrowing interest rate
    uint256 constant BASE_RATE = 5;

    /// @notice Spread represents the economic profit of the protocol
    uint256 constant SPREAD = 10;

    /// @notice Owner of the contract (Matter Labs)
    address internal owner;

    /// @notice verifier contract
    Verifier internal verifier;

    /// @notice governance contract
    Governance internal governance;

    /// @notice Franklin contract
    Franklin internal franklin;

    /// @notice last verified Fraklin block
    uint256 internal lastVerifiedBlock;

    /// @notice total funds supply on contract
    uint256 internal totalSupply;

    /// @notice total funds borrowed on contract
    uint256 internal totalBorrowed;

    /// @notice Container for information about this contracts' token
    /// @member tokenAddress Token ethereum address
    /// @member tokenId Token Franklin id
    struct Token {
        address tokenAddress;
        uint16 tokenId;
    }

    /// @notice This contracts' token
    Token public token;

    /// @notice Lenders list
    mapping(uint32 => address) internal lenders;

    /// @notice Each lenders' supply
    mapping(address => uint256) public lendersSupplies;

    /// @notice Funded supply for each lender
    uint32 internal lendersCount;

    /// @notice Fee orders by Franklin block number
    mapping(uint32 => mapping(uint32 => FeeOrder)) public blockFeeOrders;

    /// @notice Franklin blocks details by its number
    mapping(uint32 => BlockInfo) public blocksInfo;

    /// @notice Borrow orders by Franklin block number
    mapping(uint32 => mapping(uint32 => BorrowOrder)) public blockBorrowOrders;

    /// @notice Deffered withdraw orders list
    mapping(uint32 => DefferedWithdrawOrder) public defferedWithdrawOrders;

    /// @notice Index of the current first (not fulfilled) deffered withdraw order
    uint32 internal startDefferedWithdrawOrdersIndex;

    /// @notice Current withdraw orders count
    uint32 internal defferedWithdrawOrdersCount;

    /// @notice Container for information about deffered withdraw order
    /// @member amountLeft The amount that left to withdraw
    /// @member lender Order owner
    struct DefferedWithdrawOrder {
        uint256 amountLeft;
        address lender;
    }

    /// @notice Container for block details
    /// @member feeOrdersCount Fee orders count in this block
    /// @member borrowOrdersCount Borrow orders count in this block
    /// @member fee Blocks' total fee amount
    /// @member borrowed Blocks' total borrowed token amount
    struct BlockInfo {
        uint32 feeOrdersCount;
        uint32 borrowOrdersCount;
        uint256 fee;
        uint256 borrowed;
    }

    /// @notice Container for information about fee order
    /// @member fee This orders' fee
    /// @member borrowed Fee orders' recipient (lender)
    struct FeeOrder {
        uint256 fee;
        address lender;
    }

    /// @notice Container for information about borrow order
    /// @member onchainOpNumber Onchain op number
    /// @member fee This orders' amount
    /// @member borrowed Borrow orders' recipient (receiver)
    struct BorrowOrder {
        uint64 onchainOpNumber;
        uint256 amount;
        address receiver;
    }

    /// @notice Event emitted when a new deffered borrow is created
    event NewDefferedBorrowOrder(
        uint32 blockNumber,
        uint32 orderId,
        uint256 amount
    );

    /// @notice Event emitted when a deffered borrow is fulfilled
    event FulfilledDefferedBorrowOrder(
        uint32 blockNumber,
        uint32 orderId
    );

    /// @notice Event emitted when a deffered withdraw order is updated
    event UpdatedDefferedWithdrawOrder(
        uint32 orderNumber,
        address lender,
        uint256 amountLeft
    );

    /// @notice Construct a new token lending
    /// @param _tokenAddress The address of the specified token
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
    ) public {
        governance = Governance(_governanceAddress);
        uint16 tokenId = 0;
        if (_tokenAddress != address(0)) {
            tokenId = governance.validateTokenAddress(_token);
        }
        token = Token({
            tokenAddress: _tokenAddress,
            tokenId: tokenId
        });

        franklin = Franklin(_priorityQueueAddress);
        lastVerifiedBlock = Franklin.totalBlocksVerified;

        verifier = Verifier(_verifierAddress);
        owner = _owner;
    }

    /// @notice Fallback function always reverts
    function fallbackInternal() internal {
        revert("Cant accept ether through fallback function");
    }

    /// @dev Performs a transfer in
    function transferIn(uint256 _amount) internal;

    /// @dev Performs a transfer out
    function transferOut(uint256 _amount, address _to) internal;

    /// @notice Supplies specified amount of tokens from lender
    /// @dev Calls transferIn function of specified token and fulfillDefferedWithdrawOrders to fulfill deffered withdraw orders
    /// @param _amount Token amount
    /// @param _lender Lender account address
    function supplyInternal(uint256 _amount, address _lender) internal {
        transferIn(_amount);
        totalSupply = totalSupply.add(_amount);
        if (lendersSupplies[_lender] == 0) {
            lenders[lendersCount] = _lender;
            lendersCount++;
        }
        lendersSupplies[_lender] += _amount;
        fulfillDefferedWithdrawOrders();
    }

    /// @notice Fulfills deffered withdraw orders
    /// @dev The amount of supply is: totalSupply - totalBorrowed. Emits UpdatedDefferedWithdrawOrder event
    function fulfillDefferedWithdrawOrders() internal {
        uint256 amount = totalSupply - totalBorrowed;
        uint32 i = 0;
        uint32 deletedOrdersCount = 0;
        for (uint32 i = 0; i < defferedWithdrawOrdersCount; i++) {
            uint256 amountToFulfill = defferedWithdrawOrders[startDefferedWithdrawOrdersIndex + i].amountLeft;
            if (amountToFulfill >= amount) {
                defferedWithdrawOrders[startDefferedWithdrawOrdersIndex + i].amount = amountToFulfill - amount;
                emit UpdatedDefferedWithdrawOrder(
                    startDefferedWithdrawOrdersIndex + i,
                    defferedWithdrawOrders[startDefferedWithdrawOrdersIndex + i].lender,
                    amountToFulfill - amount
                );
                if (amountToFulfill == amount) {
                    deletedOrdersCount++;
                }
                break;
            } else {
                defferedWithdrawOrders[startDefferedWithdrawOrdersIndex + i].amount = 0;
                emit UpdatedDefferedWithdrawOrder(
                    startDefferedWithdrawOrdersIndex + i,
                    defferedWithdrawOrders[startDefferedWithdrawOrdersIndex + i].lender,
                    0
                );
                deletedOrdersCount++;
                amount -= amountToFulfill;
            }
        }
        startDefferedWithdrawOrdersIndex += deletedOrdersCount;
    }

    /// @notice Starts withdrawing process
    /// @dev If there is enought free tokens, calls immediateWithdraw. Else calls defferedWithdraw
    /// @param _amount Token amount
    /// @param _to Receiver address
    function requestWithdrawInternal(uint256 _amount, address _to) internal {
        require(
            lendersSupplies[msg.sender] >= _amount,
            "ltwl11"
        ); // "ltwl11" - not enouth lender supply
        if (_amount <= totalSupply - totalBorrowed) {
            immediateWithdraw(_amount, _to);
        } else {
            defferedWithdraw(_amount, _to);
        }
    }

    /// @notice The available amount of funds will be withdrawn through immediateWithdraw, for a gradual automatic withdrawal of the remaining, a `DefferedWithdrawOrder` will be created
    /// @dev Emits UpdatedDefferedWithdrawOrder event
    /// @param _amount Token amount
    /// @param _to Receiver address
    function defferedWithdraw(uint256 _amount, address _to) internal {
        uint256 immediateAmount = totalSupply - totalBorrowed;
        require(
            _amount >= immediateAmount,
            "ltdw11"
        ); // "ltwl11" - wrong amount
        immediateWithdraw(immediateAmount, _to);
        defferedWithdrawOrders[startDefferedWithdrawOrdersIndex + defferedWithdrawOrdersCount] = DefferedWithdrawOrder({
            amountLeft: immediateAmount,
            lender: _to
        });
        emit UpdatedDefferedWithdrawOrder(
            startDefferedWithdrawOrdersIndex + defferedWithdrawOrdersCount,
            _to,
            immediateAmount
        );
        defferedWithdrawOrdersCount++;
    }

    /// @notice The specified amount of funds will be withdrawn
    /// @param _amount Token amount
    /// @param _to Receiver address
    function immediateWithdraw(uint256 _amount, address _to) internal {
        transferOut(_amount, _to);
        totalSupply -= _amount;
        lendersSupplies[msg.sender] -= _amount;
        // delete
        if (lendersSupplies[msg.sender] == 0) {
            bool found = false;
            for (uint32 i = 0; i < lendersCount-2; i++){
                if (found || lenders[i] == msg.sender) {
                    found = true;
                    lenders[i] = lenders[i+1];
                }
            }
            delete lenders[lendersCount-1];
            lendersCount--;
        }
    }

    /// @notice Borrows the free funds of creditors for user to perform sending operation
    /// @dev If there is enought free tokens, calls immediateBorrow. Else calls defferedBorrow. The necessary checks will occur:
    /// - the block must be unverified
    /// - the borrowing amount must be positive
    /// - signature verification must be successful
    /// - verification of the borrow request in the specified Franklin block must
    /// @param _onchainOpNumber Franklin onchain operation number
    /// @param _amount The borrow amount
    /// @param _borrower Borrower id
    /// @param _receiver Receiver address
    /// @param _signature Borrow request signature
    function requestBorrowInternal(
        uint64 _onchainOpNumber,
        uint256 _amount,
        uint24 _borrower,
        address _receiver,
        bytes _signature
    ) internal {
        require(
            lastVerifiedBlock < _blockNumber,
            "ltrl11"
        ); // "ltrl11" - verified block
        require(
            _amount > 0,
            "ltrl12"
        ); // "ltrl12" - zero amount
        require(
            verifier.verifyBorrowSignature(_signature), // TODO: !!!!!
            "ltrl13"
        ); // "ltrl13" - wrong signature
        require(
            franklin.verifyBorrowRequest(_onchainOpNumber, _borrower, _receiver, token.tokenId, _amount),
            "ltrl14"
        ); // "ltrl14" - wrong tx
        BorrowOrder order = BorrowOrder({
            onchainOpNumber: _onchainOpNumber,
            amount: _amount,
            receiver: _receiver
        });
        if (_amount <= (totalSupply - totalBorrowed)) {
            immediateBorrow(_blockNumber, order);
        } else {
            defferedBorrow(_blockNumber, order);
        }
    }

    /// @notice Creates deffered BorrowOrder
    /// @dev Emits NewDefferedBorrowOrder
    /// @param _blockNumber The number of committed block with withdraw operation
    /// @param _order Borrow order
    function defferedBorrow(
        uint32 _blockNumber,
        BorrowOrder _order
    ) internal {
        uint32 currentBorrowOrdersCount = blocksInfo[_blockNumber].borrowOrdersCount;
        blockBorrowOrders[_blockNumber][currentBorrowOrdersCount] = _order;
        emit NewDefferedBorrowOrder(
            _blockNumber,
            currentBorrowOrdersCount,
            _order.amount
        );
        blocksInfo[_blockNumber].borrowOrdersCount++;
    }

    /// @notice Sends borrowed funds to receiver. Changes Franklin withdraw op type to lending type
    /// @dev Creates FeeOrder that will be used to provide fees to lenders when the block is verified
    /// @param _blockNumber The number of committed block with withdraw operation
    /// @param _order Borrow order
    function immediateBorrow(
        uint32 _blockNumber,
        BorrowOrder _order
    ) internal {
        (uint256 lendersFees, uint256 ownerFee) = calculateFees(_order.amount);
        transferOut((_order.amount-ownerFee-lendersFees), _order.receiver);
        for (uint32 i = 0; i <= lendersCount; i++) {
            uint32 currentFeeOrdersCount = blocksInfo[_blockNumber].feeOrdersCount;
            blockFeeOrders[_blockNumber][currentFeeOrdersCount] = FeeOrder({
                fee: lendersFees * (lendersSupplies[lenders[i]] / totalSupply),
                lender: lenders[i]
            });
            blocksInfo[_blockNumber].feeOrdersCount++;
        }
        uint32 currentFeeOrdersCount = blocksInfo[_blockNumber].feeOrdersCount;
        blockFeeOrders[_blockNumber][currentFeeOrdersCount] = FeeOrder({
            fee: ownerFee,
            lender: owner
        });
        blocksInfo[_blockNumber].feeOrdersCount++;

        blocksInfo[_blockNumber].borrowed += _order.amount-ownerFee-lendersFees;
        blocksInfo[_blockNumber].fee += ownerFee+lendersFees;

        totalBorrowed += _order.amount-ownerFee-lendersFees;

        franklin.withdrawOpToLending(token.tokenId, _order.onchainOpNumber);
    }

    /// @notice Calculates lenders and owner fees
    /// @param _amount Token amount
    /// @return Lenders fees and owner fee
    function calculateFees(uint256 _amount) internal returns (uint256 _lendersFees, uint256 _ownerFee) {
        (uint256 bir, uint256 sir) = getCurrentInterestRatesInternal();
        uint256 borrowerFee = bir * _amount;
        _lendersFees = borrowerFee * sir;
        _ownerFee = borrowerFee - lendersFees;
    }

    /// @notice Calculates current interest rates
    /// @return Borrowing and supply interest rates
    function getCurrentInterestRatesInternal() internal pure returns (uint256 _borrowing, uint256 _supply) {
        uint256 u = totalBorrowed / (totalSupply + totatBorrowed);
        _borrowing = MULTIPLIER * u + BASE_RATE;
        _supply = _borrowing * u * (1 - SPREAD);
    }

    /// @notice Fulfills deffered borrow order
    /// @dev Calls supplyInternal and immediateBorrow. Emits FulfilledDefferedBorrowOrder. The necessary checks will occur:
    /// - the block must be unverified
    /// - order id must be less than borrow orders count in this block
    /// - sending amount must be more or equal to order amount
    /// @param _blockNumber The number of committed block with withdraw operation
    /// @param _orderId Specified order id
    /// @param _sendingAmount Specified amount
    /// @param _lender Lender address
    function fulfillDefferedBorrowOrderInternal(
        uint32 _blockNumber,
        uint32 _orderId,
        uint256 _sendingAmount,
        address _lender
    ) internal {
        require(
            lastVerifiedBlock < _blockNumber,
            "ltfl11"
        ); // "ltfl11" - verified block
        require(
            _orderId < blocksInfo[_blockNumber].borrowOrdersCount,
            "ltfl12"
        ); // "ltfl12" - wrong block order number
        require(
            _sendingAmount >= blockBorrowOrders[_blockNumber][_orderId].amount,
            "ltfl13"
        ); // "ltfl13" - wrong amount
        supplyInternal(_sendingAmount, _lender);
        immediateBorrow(
            blockBorrowOrders[_blockNumber][_orderId],
            _blockNumber
        );
        delete blockBorrowOrders[_blockNumber][_orderId];
        emit FulfilledDefferedBorrowOrder(
            _blockNumber,
            _orderId
        );
    }

    /// @notice Calls by Franklin contract when a new block is verified. Removes this blocks' orders and consummates fees
    /// @param _blockNumber The number of committed block with withdraw operation
    function newVerifiedBlockInternal(uint32 _blockNumber) internal {
        requireFranklin();
        lastVerifiedBlock = _blockNumber;
        delete blockBorrowOrders[_blockNumber];
        consummateBlockFees(_blockNumber);
        delete blocksInfo[_blockNumber];
        delete blockFeeOrders[_blockNumber];
    }

    /// @notice Consummates block fees, frees borrowed funds
    /// @dev Calls fulfillDefferedWithdrawOrders
    /// @param _blockNumber The number of committed block with withdraw operation
    function consummateBlockFees(uint32 _blockNumber) internal {
        for (uint32 i = 0; i < blocksInfo[_blockNumber].feeOrdersCount; i++) {
            address feeOrder = blockFeeOrders[_blockNumber][i];
            if (lendersSupplies[feeOrder.lender] == 0) {
                lenders[lendersCount] = feeOrder.lender;
                lendersCount++;
            }
            lendersSupplies[feeOrder.lender] += feeOrder.fee;
        }
        totalSupply = totalSupply.add(blocksInfo[_blockNumber].fee);
        totalBorrowed -= blocksInfo[_blockNumber].borrowed;
        fulfillDefferedWithdrawOrders();
    }

    /// @notice Calls by Franklin contract to provide borrowed fees from withdraw operation
    /// @param _amount The amount specified in withdraw operation
    function repayBorrowInternal(uint256 _amount) internal {
        requireFranklin();
        transferIn(_amount);
    }

    /// @notice Check if the sender is franklin contract
    function requireFranklin() internal view {
        require(
            msg.sender == franklinAddress,
            "ltrn11"
        ); // ltrn11 - only by franklin
    }
}
