pragma solidity ^0.5.8;

import "./Governance.sol";
import "./Franklin.sol";
import "./Verifier.sol";

contract LendingToken {
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

    event NewBorrowOrder(
        uint32 blockNumber,
        uint32 orderId,
        uint256 amount
    );

    event NewBorrowOrder(
        uint32 blockNumber,
        uint32 orderId,
        uint256 amount
    );

    event FulfilledBorrowOrder(
        uint32 blockNumber,
        uint32 orderId
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

    function supplyInternal(uint256 _amount, address _to) internal {
        transferIn(_amount);
        totalSupply += _amount;
        if (lendersSupplies[_to] == 0) {
            lenders[lendersCount] = _to;
            lendersCount++;
        }
        lendersSupplies[_to] += _amount;
    }

    function transferIn(uint256 _amount) internal;

    function withdrawInternal(uint256 _amount, address _to) internal {
        require(
            lendersSupplies[msg.sender] >= _amount,
            "lww11"
        ); // "lww11" - not enouth lender supply
        require(
            _amount <= totalSupply - totalBorrowed,
            "lww12"
        ); // "lww12" - not enouth availabl supplies
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
        bytes32 _txHash,
        bytes _signature,
        uint256 _amount,
        address _borrower,
        address _receiver,
        uint32 _blockNumber
    ) internal {
        require(
            lastVerifiedBlock < _blockNumber,
            "lrw11"
        ); // "lrw11" - verified block
        require(
            _amount > 0,
            "lrw11"
        ); // "lrw11" - zero amount
        require(
            verifySignature(_txHash, _signature),
            "lrw11"
        ); // "lrw11" - wrong signature
        require(
            verifyTx(_amount, token.tokenAddress, _borrower, _blockNumber, _txHash),
            "lrw12"
        ); // "lrw12" - wrong tx
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
        emit NewBorrowOrder(
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
            "lrw11"
        ); // "lrw11" - verified block
        require(
            _orderId < blocksInfo[_blockNumber].borrowOrdersCount,
            "lrw11"
        ); // "lrw11" - wrong block order number
        require(
            _sendingAmount >= blockBorrowOrders[_blockNumber][_orderId].amount,
            "lrw11"
        ); // "lrw11" - wrong amount
        supplyInternal(_sendingAmount, _lender);
        immediateBorrow(
            blockBorrowOrders[_blockNumber][_orderId].amount,
            blockBorrowOrders[_blockNumber][_orderId].receiver,
            _blockNumber
        );
        delete blockBorrowOrders[_blockNumber][_orderId];
        emit FulfilledBorrowOrder(
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
        totalSupply += blocksInfo[_blockNumber].fee;
        totalBorrowed -= blocksInfo[_blockNumber].borrowed;
    }

    // Check if the sender is franklin contract
    function requireFranklin() internal view {
        require(
            msg.sender == franklinAddress,
            "prn11"
        ); // prn11 - only by franklin
    }
}
