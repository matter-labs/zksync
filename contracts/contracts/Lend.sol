pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Governance.sol";
import "./Franklin.sol";
import "./Verifier.sol";

contract Token {
    address owner;

    uint256 multiplier;
    uint256 baseRate;
    uint256 spread;

    uint256 lastVerifiedBlock;

    uint256 totalSupply;
    uint256 matterFee;
    uint256 totalBorrowed;

    mapping(uint32 => address) public lenders;
    mapping(address => uint256) public lendersSupplies;
    uint32 lendersCount;

    mapping(uint32 => mapping(uint32 => FeeOrder)) blockFeeOrders;
    mapping(uint32 => BlockInfo) blocksInfo;
    mapping(uint32 => mapping(uint32 => BorrowOrder)) blockBorrowOrders;

    struct BlockInfo {
        uint32 feeOrdersCount;
        uint32 borrowOrdersCount;
        uint256 totalFee;
        uint256 totalBorrowed;
    }

    struct FeeOrder {
        uint256 fee;
        address lender;
    }

    struct BorrowOrder {
        uint256 amount;
        address receiver;
    }

    Verifier internal verifier;
    Governance internal governance;
    Franklin internal franklin;

    constructor(
        uint256 _multiplier,
        uint256 _baseRate,
        uint256 _spread,
        address _governanceAddress,
        address _franklinAddress,
        address _verifierAddress,
        address _owner
    ) public {
        governance = Governance(_governanceAddress);
        franklin = Franklin(_priorityQueueAddress);
        verifier = Verifier(_verifierAddress);
        owner = _owner;
        multiplier = _multiplier;
        baseRate = _baseRate;
        spread = _spread;
    }

    function supply(uint256 _amount, address _lender) external {
        transaction(_amount, address(this));
        totalSupply += _amount;
        lendersSupplies[_lender] += _amount;
        lenders[lendersCount] = _lender;
        lendersCount++;
    }

    function withdraw(uint256 _amount) external {
        require(
            lendersSupplies[msg.sender] >= _amount,
            "lww11"
        ); // "lww11" - not enouth lender supply
        require(
            _amount <= totalSupply - totalBorrowed,
            "lww12"
        ); // "lww12" - not enouth availabl supplies
        
        transaction(_amount, msg.sender);
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

    function requestBorrow(
        bytes32 _txHash,
        bytes _signature,
        uint256 _amount,
        address _borrower,
        address _receiver,
        uint32 _blockNumber
    ) external {
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
            verifyTx(_amount, _borrower, _blockNumber, _txHash)
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
        transaction((_amount-ownerFee-lendersFees), _receiver);
        BlockInfo blockInfo = blocksInfo[_blockNumber];
        for (uint32 i = 0; i <= lendersCount; i++) {
            blockFeeOrders[_blockNumber][blocksInfo[_blockNumber].feeOrdersCount] = FeeOrder({
                lendersFees * (lendersSupplies[lenders[i]] / totalSupply),
                lenders[i]
            });
            blocksInfo[_blockNumber].feeOrdersCount++;
        }
        blockFeeOrders[_blockNumber][blocksInfo[_blockNumber].feeOrdersCount] = FeeOrder({
            ownerFee,
            owner
        });
        blocksInfo[_blockNumber].feeOrdersCount++;

        blocksInfo[_blockNumber].totalBorrowed += _amount-ownerFee-lendersFees;
        blocksInfo[_blockNumber].totalFee += ownerFee+lendersFees;

        totalBorrowed += _amount-ownerFee-lendersFees;
    }

    function calculateFees(uint256 _amount) internal returns (uint256 lendersFees, uint256 ownerFee) {
        // TODO: - safe math
        uint256 u = totalBorrowed / (totalSupply + totatBorrowed);
        uint256 bir = multiplier * u + baseRate;
        uint256 borrowerFee = bir * _amount;
        uint256 sir = bir * u * (1 - spread);
        lendersFees = borrowerFee * sir;
        ownerFee = borrowerFee - lendersFees;
    }

    function differedBorrow(
        uint256 _amount,
        address _receiver,
        uint32 _blockNumber
    ) internal {
        blockBorrowOrders[_blockNumber][blocksInfo[_blockNumber].borrowOrdersCount] = BorrowOrder({
            _amount,
            _receiver
        });
        emit NewBorrowOrder(
            _amount,
            _receiver,
            _blockNumber,
            blocksInfo[_blockNumber].borrowOrdersCount
        );
        blocksInfo[_blockNumber].borrowOrdersCount++;
    }

    function fulfillOrder(uint32 _blockNumber, uint32 _orderNumber, uint256 _sendingAmount, address _lender) external {
        require(
            lastVerifiedBlock < _blockNumber,
            "lrw11"
        ); // "lrw11" - verified block
        require(
            _orderNumber < blocksInfo[_blockNumber].borrowOrdersCount,
            "lrw11"
        ); // "lrw11" - wrong block order number
        require(
            _sendingAmount >= blockBorrowOrders[_blockNumber][_orderNumber].amount,
            "lrw11"
        ); // "lrw11" - wrong amount
        supply(_sendingAmount, _lender);
        immediateBorrow(
            blockBorrowOrders[_blockNumber][_orderNumber].amount,
            blockBorrowOrders[_blockNumber][_orderNumber].receiver,
            _blockNumber
        );
        delete blockBorrowOrders[_blockNumber][_orderNumber];
        emit FulfilledBorrowOrder(
            _blockNumber,
            _orderNumber
        );
    }

    function newVerifiedBlock(uint32 _blockNumber) external {
        requireFranklin();
        lastVerifiedBlock = _blockNumber;
        delete blockBorrowOrders[_blockNumber];
        consummateBlockFees(_blockNumber);
        delete blocksInfo[_blockNumber];
    }

    function consummateBlockFees(uint32 _blockNumber) internal {
        for (uint32 i = 0; i < blocksInfo[_blockNumber].feeOrdersCount; i++) {
            lendersSupplies[blockFeeOrders[_blockNumber][i].lender] += blockFeeOrders[_blockNumber][i].fee;
        }
        totalSupply += blocksInfo[_blockNumber].totalFee;
        totalBorrowed -= blocksInfo[_blockNumber].totalBorrowed;
    }

    // Check if the sender is franklin contract
    function requireFranklin() internal view {
        require(
            msg.sender == franklinAddress,
            "prn11"
        ); // prn11 - only by franklin
    }
}
