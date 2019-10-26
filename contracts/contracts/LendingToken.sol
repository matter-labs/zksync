pragma solidity ^0.5.8;

import "./Governance.sol";
import "./Franklin.sol";
import "./Verifier.sol";
import "./SafeMath.sol";

contract LendingToken {
    using SafeMath for uint256;

    address internal owner;
    Token public token;

    uint256 constant MULTIPLIER = 45;
    uint256 constant BASE_RATE = 5;
    uint256 constant SPREAD = 10;

    uint256 internal lastVerifiedBlock;

    uint256 internal totalSupply;
    uint256 internal totalBorrowed;

    mapping(uint32 => address) internal lenders;
    mapping(address => uint256) public lendersSupplies;
    uint32 internal lendersCount;

    mapping(uint32 => mapping(uint32 => FeeOrder)) public blockFeeOrders;
    mapping(uint32 => BlockInfo) public blocksInfo;
    mapping(uint32 => mapping(uint32 => BorrowOrder)) public blockBorrowOrders;

    mapping(uint32 => DefferedWithdrawOrder) public defferedWithdrawOrders;
    uint32 internal startDefferedWithdrawOrdersIndex;
    uint32 internal defferedWithdrawOrdersCount;

    struct DefferedWithdrawOrder {
        uint256 amountLeft;
        address lender;
    }

    struct Token {
        address tokenAddress;
        uint16 tokenId;
    }

    struct BlockInfo {
        uint32 feeOrdersCount;
        uint32 borrowOrdersCount;
        uint256 fee;
        uint256 borrowed;
    }

    struct FeeOrder {
        uint256 fee;
        address lender;
    }

    struct BorrowOrder {
        uint256 amount;
        address receiver;
    }

    event NewDefferedBorrowOrder(
        uint32 blockNumber,
        uint32 orderId,
        uint256 amount
    );

    event FulfilledDefferedBorrowOrder(
        uint32 blockNumber,
        uint32 orderId
    );

    event UpdatedDefferedWithdrawOrder(
        uint32 orderNumber,
        address lender,
        uint256 amountLeft
    );

    Verifier internal verifier;
    Governance internal governance;
    Franklin internal franklin;

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

    function fallbackInternal() internal {
        revert("Cant accept ether through fallback function");
    }

    function supplyInternal(uint256 _amount, address _to) internal {
        transferIn(_amount);
        totalSupply = totalSupply.add(_amount);
        if (lendersSupplies[_to] == 0) {
            lenders[lendersCount] = _to;
            lendersCount++;
        }
        lendersSupplies[_to] += _amount;
        fulfillDefferedWithdrawOrders();
    }

    function transferIn(uint256 _amount) internal;

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

    function transferOut(uint256 _amount, address _to) internal;

    function requestBorrowInternal(
        uint32 _blockNumber,
        uint32 _requestNumber,
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
            verifier.verifyBorrowSignature(_txNumber, _signature),
            "ltrl13"
        ); // "ltrl13" - wrong signature
        require(
            franklin.verifyBorrowRequest(_blockNumber, _requestNumber, _borrower, token.tokenId, _amount),
            "ltrl14"
        ); // "ltrl14" - wrong tx
        if (_amount <= (totalSupply - totalBorrowed)) {
            immediateBorrow(_amount, _receiver, _blockNumber);
        } else {
            defferedBorrow(_amount, _receiver, _blockNumber);
        }
    }

    function immediateBorrow(
        uint256 _amount,
        address _receiver,
        uint32 _blockNumber
    ) internal {
        (uint256 lendersFees, uint256 ownerFee) = calculateFees(_amount);
        transferOut((_amount-ownerFee-lendersFees), _receiver);
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

        blocksInfo[_blockNumber].borrowed += _amount-ownerFee-lendersFees;
        blocksInfo[_blockNumber].fee += ownerFee+lendersFees;

        totalBorrowed += _amount-ownerFee-lendersFees;
    }

    function calculateFees(uint256 _amount) internal returns (uint256 _lendersFees, uint256 _ownerFee) {
        // TODO: - safe math
        (uint256 bir, uint256 sir) = getCurrentInterestRatesInternal();
        uint256 borrowerFee = bir * _amount;
        _lendersFees = borrowerFee * sir;
        _ownerFee = borrowerFee - lendersFees;
    }

    function getCurrentInterestRatesInternal() internal pure returns (uint256 _borrowing, uint256 _supply) {
        // TODO: - safe math
        uint256 u = totalBorrowed / (totalSupply + totatBorrowed);
        _borrowing = MULTIPLIER * u + BASE_RATE;
        _supply = _borrowing * u * (1 - SPREAD);
    }

    function defferedBorrow(
        uint256 _amount,
        address _receiver,
        uint32 _blockNumber
    ) internal {
        uint32 currentBorrowOrdersCount = blocksInfo[_blockNumber].borrowOrdersCount;
        blockBorrowOrders[_blockNumber][currentBorrowOrdersCount] = BorrowOrder({
            amount: _amount,
            receiver: _receiver
        });
        emit NewDefferedBorrowOrder(
            _blockNumber,
            currentBorrowOrdersCount,
            _amount
        );
        blocksInfo[_blockNumber].borrowOrdersCount++;
    }

    function fulfillOrderInternal(
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
            blockBorrowOrders[_blockNumber][_orderId].amount,
            blockBorrowOrders[_blockNumber][_orderId].receiver,
            _blockNumber
        );
        delete blockBorrowOrders[_blockNumber][_orderId];
        emit FulfilledDefferedBorrowOrder(
            _blockNumber,
            _orderId
        );
    }

    function newVerifiedBlockInternal(uint32 _blockNumber) internal {
        requireFranklin();
        lastVerifiedBlock = _blockNumber;
        delete blockBorrowOrders[_blockNumber];
        consummateBlockFees(_blockNumber);
        delete blocksInfo[_blockNumber];
        delete blockFeeOrders[_blockNumber];
    }

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

    function repayBorrowInternal(uint256 _amount) internal {
        requireFranklin();
        transferIn(_amount);
    }

    // Check if the sender is franklin contract
    function requireFranklin() internal view {
        require(
            msg.sender == franklinAddress,
            "ltrn11"
        ); // ltrn11 - only by franklin
    }
}
