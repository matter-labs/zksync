pragma solidity ^0.5.8;

import "./Governance.sol";
import "./Franklin.sol";
import "./SafeMath.sol";
import "./BlsOperations.sol";

/// @title Lending Token Contract
/// @notice Inner logic for LendingEther and LendingErc20 token contracts
/// @author Matter Labs
contract SwiftExitsSingle {
    
    using SafeMath for uint256;

    /// @notice Possible price change coeff on Compound
    uint256 constant possiblePriceRisingCoeff = 110/100;

    /// @notice Validators fee
    uint256 constant validatorsFeeCoeff = 95 / 100;

    /// @notice Owner of the contract (Matter Labs)
    address internal owner;

    /// @notice Matter token contract address
    address internal matterToken;

    /// @notice Governance contract
    Governance internal governance;

    /// @notice Rollup contract
    Franklin internal rollup;

    /// @notice Comptroller contract
    Comptroller internal comptroller;

    /// @notice last verified Fraklin block
    uint256 internal lastVerifiedBlock;

    mapping(uint16 => uint256) internal supplies;
    mapping(uint16 => uint256) internal borrows;

    /// @dev Possible exit order states
    enum ExitOrderState {
        None,
        Deffered,
        Fulfilled
    }

    /// @notice Swift Exits hashes (withdraw op hash) by Rollup block number
    mapping(uint32 => mapping(uint32 => uint256)) internal exitOrdersHashes;
    /// @notice Swift Exits in blocks count
    mapping(uint32 => uint32) internal exitOrdersCount;
    /// @notice Swift Exits by its hash (withdraw op hash)
    mapping(uint256 => ExitOrder) internal exitOrders;

    /// @notice Borrow orders by Swift Exits hashes (withdraw op hash)
    mapping(uint256 => BorrowOrder) internal borrowOrders;

    /// @notice Container for information about borrow order
    /// @member borrowed This orders' borrow
    struct BorrowOrder {
        uint16 tokenId;
        uint256 borrowed;
        uint256 fee;
    }

    /// @notice Container for information about Swift Exit Order
    /// @member status Order status
    /// @member withdrawOpOffset Withdraw operation offset in block
    /// @member tokenId Order token id
    /// @member tokenAmount Order token amount
    /// @member supplyAmount Order supply amount
    /// @member recipient Recipient address
    struct ExitOrder {
        ExitOrderState status;
        uint64 withdrawOpOffset;
        uint16 tokenId;
        uint256 tokenAmount;
        uint16 tokenSupplyId;
        uint256 supplyAmount;
        address recipient;
    }

    /// @notice Emitted when a new swift exit order occurs
    event UpdatedExitOrder(
        uint32 blockNumber,
        uint32 orderNumber,
        uint256 expectedAmountToSupply
    );

    /// @notice Construct swift exits contract
    /// @param _owner The address of this contracts owner (Matter Labs)
    constructor(address _owner) public {
        owner = _owner;
    }

    /// @notice Add addresses of related contracts
    /// @dev Requires owner
    /// @param _matterTokenAddress The address of Matter token
    /// @param _governanceAddress The address of Governance contract
    /// @param _rollupAddress The address of Rollup contract
    /// @param _blsVerifierAddress The address of Bls Verifier contract
    /// @param _comptrollerAddress The address of Comptroller contract
    function setupRelatedContracts(
        address _matterTokenAddress,
        address _governanceAddress,
        address _rollupAddress,
        address _comptrollerAddress
    ) external {
        requireOwner();
        governance = Governance(_governanceAddress);
        rollup = Franklin(_rollupAddress);
        lastVerifiedBlock = rollup.totalBlocksVerified;
        comptroller = Comptroller(_comptrollerAddress);
        matterToken = _matterTokenAddress;
    }

    /// @notice Fallback function always reverts
    function() external {
        revert("Cant accept ether through fallback function");
    }

    /// @notice Transfers specified amount of ERC20 token into contract
    /// @param _from Sender address
    /// @param _tokenAddress ERC20 token address
    /// @param _amount Amount in ERC20 tokens
    function transferInERC20(address _from, address _tokenAddress, uint256 _amount) internal {
        require(
            IERC20(tokenAddr).transferFrom(_from, address(this), _amount),
            "sst011"
        ); // sst011 - token transfer in failed
    }

    /// @notice Transfers specified amount of Ether or ERC20 token to external address
    /// @dev If token id == 0 -> transfers ether
    /// @param _to Receiver address
    /// @param _tokenAddress ERC20 token address
    /// @param _amount Amount in ERC20 tokens
    function transferOut(address _to, address _tokenAddress, uint256 _amount) internal {
        if (_tokenAddress == address(0)) {
            _to.transfer(_amount);
        } else {
            require(
                IERC20(_tokenAddress).transfer(_to, _amount),
                "sstt11"
            ); // sstt11 - token transfer out failed
        }
    }

    /// @notice Gets allowed withdraw amount
    /// @param _tokenId Specified token id
    function getAllowedWithdrawAmount(uint256 _tokenId) public returns (uint256) {
        requireOwner();
        return supplies[_tokenId]-borrows[_tokenId];
    }

    /// @notice Withdraws specified amount from validators supply
    /// @dev Requires validators' existance and allowed amount is >= specified amount, which should not be equal to 0
    /// @param _amount Specified amount
    /// @param _tokenId Specified token id
    function withdraw(uint256 _amount, uint256 _tokenId) external {
        requireOwner();
        require(
            getAllowedWithdrawAmount(_tokenId) >= _amount,
            "sswr11"
        ); // sswr11 - wrong amount
        immediateWithdraw(_amount, _tokenId);
    }

    /// @notice Withdraws possible amount from validators supply
    /// @dev Requires validators' existance and allowed amount > 0
    /// @param _tokenId Specified token id
    function withdrawPossible(uint256 _tokenId) external {
        requireOwner();
        uint256 amount = getAllowedWithdrawAmount(_tokenId);
        immediateWithdraw(amount, _tokenId);
    }

    /// @notice The specified amount of funds will be withdrawn from validators supply
    /// @param _amount Specified amount
    /// @param _tokenId Specified token id
    function immediateWithdraw(uint256 _amount, uint256 _tokenId) internal {
        require(
            _amount > 0,
            "ssir11"
        ); // ssir11 - wrong amount
        transferOut(msg.sender, governance.tokenAddress(_tokenId), _amount);
        supplies[_tokenId] -= _amount;
    }

    /// @notice Supplies specified amount of tokens from validator
    /// @dev Calls transferIn function of specified token and fulfillDefferedWithdrawOrders to fulfill deffered withdraw orders
    /// @param _amount Token amount
    /// @param _tokenId Specified token id
    function supply(address _address, uint256 _amount, uint256 _tokenId) public {
        transferInERC20(_address, governance.tokenAddress(_tokenId), _amount);
        supplies[_tokenId] = supplies[_tokenId].add(_amount);
    }

    /// @notice Adds new swift exit
    /// @dev Only owner can send this order, validates validators signature, requires that order must be new (status must be None)
    /// @param _blockNumber Rollup block number
    /// @param _withdrawOpOffset Withdraw operation offset in block
    /// @param _withdrawOpHash Withdraw operation hash
    /// @param _tokenId Token id
    /// @param _tokenAmount Token amount
    /// @param _recipient Swift exit recipient
    /// @param _aggrSignatureX Aggregated validators signature x
    /// @param _aggrSignatureY Aggregated validators signature y
    /// @param _validators Validators addresses list
    function addSwiftExit(
        uint32 _blockNumber,
        uint64 _withdrawOpOffset,
        uint256 _withdrawOpHash,
        uint16 _tokenId,
        uint256 _tokenAmount,
        address _recipient,
        uint256 _aggrSignatureX,
        uint256 _aggrSignatureY,
        address[] calldata _validators
    ) external {
        requireOwner();
        require(
            exitOrders[_withdrawOpHash].status == ExitOrderState.None,
            "ssat13"
        ); // "ssat13" - request exists
        
        if (_blockNumber <= _lastVerifiedBlock) {
            // If block is already verified - try to send requested funds to recipient on rollup contract
            rollup.trySwiftExitWithdraw(
                _blockNumber,
                _withdrawOpOffset,
                _withdrawOpHash,
                _recipient
            );
        } else {
            // Get amount to borrow
            (uint16 tokenSupplyId, uint256 amountTokenSupply, uint256 amountTokenBorrow, uint256 ownerFee) = getAmountsAndFees(_tokenId, _tokenAmount);

            // Create Exit orer
            ExitOrder order = ExitOrder(
                ExitOrderState.None,
                _withdrawOpOffset,
                _tokenId,
                amountTokenBorrow,
                tokenSupplyId,
                amountTokenSupply,
                _recipient
            );
            exitOrdersHashes[_blockNumber][exitOrdersCount[_blockNumber]] = _withdrawOpHash;
            exitOrders[_withdrawOpHash] = order;
            exitOrdersCount[_blockNumber]++;

            if (amountTokenSupply <= (supplies[tokenSupplyId] - borrows[tokenSupplyId])) {
                // If amount to borrow <= borrowable amount - immediate swift exit
                immediateSwiftExit(_blockNumber, _withdrawOpHash, ownerFee);
            } else {
                // If amount to borrow > borrowable amount - deffered swift exit order
                exitOrders[_withdrawOpHash].status = ExitOrderState.Deffered;
                emit UpdatedExitOrder(
                    _blockNumber,
                    _withdrawOpHash,
                    (amountTokenSupply * possiblePriceRisingCoeff) - supplies[tokenSupplyId] + borrows[tokenSupplyId]
                );
            }
        }
    }

    /// @notice Supplies validators balance and immediatly fulfills swift exit order if possible
    /// @dev Requires that order must be deffered (status must be Deffered) and block must be unverified
    /// @param _blockNumber Rollup block number
    /// @param _withdrawOpHash Withdraw operation hash
    /// @param _sendingAmount Sending amount
    function supplyAndFulfillSwiftExitOrder(
        uint32 _blockNumber,
        uint32 _withdrawOpHash,
        uint256 _sendingAmount
    ) external {
        require(
            _blockNumber > lastVerifiedBlock,
            "ssfl11"
        ); // "ssfl11" - block is already verified
        require(
            exitOrders[_withdrawOpHash].status == ExitOrderState.Deffered,
            "ssfl12"
        ); // "ssfl12" - not deffered order

        supplyValidator(msg.sender, _sendingAmount);
        
        ExitOrder order = exitOrders[_withdrawOpHash];

        uint256 updatedAmount = 0;
        (uint16 tokenSupplyId, uint256 amountTokenSupply, uint256 amountTokenBorrow, uint256 ownerFee) = getAmountsAndFees(order.tokenId, order.tokenAmount);

        exitOrders[_withdrawOpHash].tokenSupplyId = tokenSupplyId;
        exitOrders[_withdrawOpHash].tokenAmount = amountTokenBorrow;
        exitOrders[_withdrawOpHash].supplyAmount = amountTokenSupply;

        if (amountTokenSupply <= (supplies[tokenSupplyId] - borrows[tokenSupplyId])) {
            // If amount to borrow <= borrowable amount - immediate swift exit
            immediateSwiftExit(_blockNumber, _withdrawOpHash, ownerFee);
        } else {
            // If amount to borrow > borrowable amount - emit update deffered swift exit order event
            updatedAmount = (amountTokenSupply * possiblePriceRisingCoeff) - supplies[tokenSupplyId] + borrows[tokenSupplyId];
        }
        emit UpdatedExitOrder(
            _blockNumber,
            _withdrawOpHash,
            updatedAmount
        );
    }

    /// @notice Processes immediatly swift exit
    /// @dev Exhanges tokens with compound, transfers token to recipient and creades swift order on rollup contract
    /// @param _blockNumber Rollup block number
    /// @param _withdrawOpHash Withdraw operation hash
    /// @param _ownerFee Amount of owner fee
    function immediateSwiftExit(
        uint32 _blockNumber,
        uint256 _withdrawOpHash,
        uint256 _ownerFee
    ) internal {
        ExitOrder order = exitOrders[_withdrawOpHash];

        borrowFromCompound(order.supplyTokenId, order.supplyAmount, order.tokenId, order.tokenAmount);
        
        exchangeWithRecipient(order.recipient, order.tokenId, order.tokenAmount);

        createBorrowOrder(_withdrawOpHash, order.supplyTokenId, order.supplyAmount, _ownerFee);

        rollup.orderSwiftExit(_blockNumber, order.withdrawOpOffset, _withdrawOpHash, order.recipient);

        exitOrders[_withdrawOpHash].status = ExitOrderState.Fulfilled;
        borrows[order.tokenSupplyId] += order.supplyAmount;
    }

    /// @notice Exchanges specified amount of token with recipient
    /// @param _recipient Recipient address
    /// @param _tokenId Token id
    /// @param _amount Token amount
    function exchangeWithRecipient(address _recipient, uint16 _tokenId, uint256 _amount) internal {
        address tokenAddress = governance.tokenAddresses(_tokenId);
        transferOut(_recipient, tokenAddress, _amount);
    }

    /// @notice Borrows specified amount from compound
    /// @param _tokenSupplyId Token supply id
    /// @param _amountTokenSupply Amount to borrow from validators and exchange with compound
    /// @param _tokenBorrowId Token borrow id
    /// @param _amountTokenBorrow Amount to get from compound
    function borrowFromCompound(
        uint16 _tokenSupplyId,
        uint256 _amountTokenSupply,
        uint16 _tokenBorrowId,
        uint256 _amountTokenBorrow
    ) internal {
        address supplyTokenAddress = governance.tokenAddresses(_tokenSupplyId);
        address cSupplyTokenAddress = governance.cTokenAddresses(supplyTokenAddress);

        address tokenAddress = governance.tokenAddresses(_tokenBorrowId);
        address cTokenAddress = governance.cTokenAddresses(tokenAddress);

        address[] memory ctokens = new address[](2);
        ctokens[0] = governance.cTokenAddresses(cSupplyTokenAddress);
        ctokens[1] = governance.cTokenAddresses(cTokenAddress);
        uint[] memory errors = comptroller.enterMarkets(ctokens);
        require(
            errors[0] == 0 && errors[1] == 0,
            "fw011"
        );  // fw011 - token approve failed

        if (_tokenSupplyId == 0) {
            CEther cToken = CEther(cSupplyTokenAddress);
            require(
                cToken.mint.value(_amountTokenSupply)() == 0,
                "got collateral?"
            );
        } else {
            uint256 allowence = IERC20(supplyTokenAddress).allowence(address(this), address(cSupplyTokenAddress));
            if (allowence < _amountTokenSupply) {
                require(
                    IERC20(supplyTokenAddress).approve(address(cSupplyTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "fw011"
                ); // fw011 - token approve failed
            }
            CErc20 cToken = CErc20(cSupplyTokenAddress);
            require(
                cToken.mint(_amountTokenSupply) == 0,
                "fw011"
            ); // fw011 - token mint failed
        }

        if (_tokenBorrowId == 0) {
            CEther cToken = CEther(cTokenAddress);
            require(
                cToken.borrow.value(_amountTokenBorrow)() == 0,
                "got collateral?"
            );
        } else {
            CErc20 cToken = CErc20(cTokenAddress);
            require(
                cToken.borrow(_amountTokenBorrow) == 0,
                "got collateral?"
            );
        }
    }

    /// @notice Repays specified amount to compound
    /// @param _tokenBorrowId Token borrow id
    /// @param _borrowAmount Amount of tokens to repay
    /// @param _tokenSupplyId Token supply id
    /// @param _supplyAmount Amount of supplied tokens
    function repayToCompound(
        uint16 _tokenBorrowId,
        uint256 _borrowAmount,
        uint16 _tokenSupplyId,
        uint256 _supplyAmount
    ) internal {
        address tokenAddress = governance.tokenAddresses(_tokenBorrowId);
        address cTokenAddress = governance.cTokenAddresses(tokenAddress);

        if (_tokenBorrowId == 0) {
            CEther cToken = CEther(cTokenAddress);
            require(
                cToken.repayBorrow.value(_borrowAmount)() == 0,
                "transfer approved?"
            );
        } else {
            uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(cTokenAddress));
            if (allowence < _borrowAmount) {
                require(
                    IERC20(tokenAddress).approve(address(cTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "fw011"
                ); // fw011 - token approve failed
            }
            CErc20 cToken = CErc20(cTokenAddress);
            require(
                cToken.repayBorrow(_borrowAmount) == 0,
                "transfer approved?"
            );
        }

        address supplyTokenAddress = governance.tokenAddresses(_tokenSupplyId);
        address cSupplyTokenAddress = governance.cTokenAddresses(supplyTokenAddress);

        if (_tokenSupplyId == 0) {
            CEther cToken = CEther(cSupplyTokenAddress);
            require(
                cToken.redeemUnderlying(_supplyAmount) == 0,
                "got collateral?"
            );
        } else {
            CErc20 cToken = CErc20(cSupplyTokenAddress);
            require(
                cToken.redeemUnderlying(_supplyAmount) == 0,
                "got collateral?"
            );
        }
    }

    /// @notice Returns amound of validators supply to exchange for token, calculates fees
    /// @param _tokenId Token id
    /// @param _tokenAmount Token amount
    function getAmountsAndFees(
        uint16 _tokenId,
        uint256 _tokenAmount
    ) internal returns (uint16 tokenSupplyId, uint256 amountTokenSupply, uint256 amountTokenBorrow, uint256 ownerFee) {
        // address tokenAddress = governance.tokenAddresses(_tokenId);
        // address cTokenAddress = governance.cTokenAddresses(tokenAddress);
        // address cMatterAddress = governance.cTokenAddresses(matterToken);

        // (bool isListed, uint collateralFactorMantissa) = comptroller.markets(cTokenAddress);

        // CErc20 cToken = CToken(cTokenAddress);
        // uint256 exchangeRateMantissa = cToken.exchangeRateCurrent();

        // uint256 collateralFactor = comptroller.takeCollateralFactor(matterAddress);

        // uint256 tokenPrice = comptroller.takePrice(tokenAddress);
        
        // uint256 fullAmount = _tokenAmount * tokenPrice / collateralFactor;

        // amountToExchange = 0.9 * fullAmount;
        // validatorsFee = 0.095 * _tokenAmount;
        // ownerFee = 0.095 * _tokenAmount;
    }

    /// @notice Creates borrow orders for specified exit request
    /// @param _withdrawOpHash Withdraw operation hash
    /// @param _tokenSupplyId Token supply id
    /// @param _amountToSupply Amount of validators supply to borrow
    /// @param _validatorsFee Amount of validators fee
    /// @param _ownerFee Amount of owner fee
    function createBorrowOrders(
        uint256 _withdrawOpHash,
        uint16 _tokenSupplyId,
        uint256 _amountToSupply,
        uint256 _ownerFee
    ) internal {
        borrowOrders[_withdrawOpHash] = BorrowOrder({
            tokenId: _tokenSupplyId,
            borrowed: _amountToSupply,
            fee: _ownerFee
        });
    }

    /// @notice Called by Rollup contract when a new block is verified. Completes swift orders process
    /// @dev Requires Rollup contract as caller. Transacts tokens and ether, repays for compound, fulfills deffered orders, punishes for failed exits
    /// @param _blockNumber The number of verified block
    /// @param _succeededHashes The succeeds exists hashes list
    /// @param _failedHashes The failed exists hashes list
    /// @param _tokenAddresses Repaid tokens
    /// @param _tokenAmounts Repaid tokens amounts
    function newVerifiedBlock(
        uint32 _blockNumber,
        uint256[] calldata _succeededHashes,
        uint256[] calldata _failedHashes,
        uint16[] calldata _tokenIds,
        uint256[] calldata _tokenAmounts
    ) external payable {
        requireRollup();
        require(
            _tokenAddresses.length == _tokenAmounts.length,
            "ssnk11"
        ); // "ssnk11" - token addresses array length must be equal token amounts array length
        for (uint i = 0; i < token.length; i++) {
            address tokenAddress = governance.tokenAddresses(_tokenIds[i]);
            transferInERC20(msg.sender, tokenAddress, _tokenAmounts[i]);
        }
        lastVerifiedBlock = _blockNumber;
        fulfillSuccededOrders(_succeededHashes);
        punishForFailedOrders(_failedHashes);
        fulfillDefferedExitOrders(_blockNumber);
    }

    /// @notice Fulfills all succeeded orders
    /// @dev Repays to compound and reduces total borrowed, sends fee to validators balance on compound
    /// @param _succeededHashes Succeeded orders hashes
    function fulfillSuccededOrders(uint256[] memory _succeededHashes) internal {
        for (uint32 i = 0; i < _succeededHashes.length; i++) {
            ExitOrder exitOrder = exitOrders[_succeededHashes[i]];
            repayToCompound(exitOrder.tokenId, exitOrder.tokenAmount, exitOrder.tokenSupplyId, exitOrder.supplyAmount);
            BorrowOrder bOrder = borrowOrders[_succeededHashes[i]];
            borrows[exitOrder.supplyTokenId] -= bOrder.borrowed;
            supplies[exitOrder.tokenId] += bOrder.fee;
        }
    }

    /// @notice Fulfills all deffered orders
    /// @dev Instantly sends from rollup to recipient for all deffered orders from specified block
    /// @param _blockNumber Block number
    function fulfillDefferedExitOrders(uint32 _blockNumber) internal {
        for (uint32 i = 0; i < exitOrdersCount[_blockNumber]; i++) {
            ExitOrder order = exitOrders[exitOrdersHashes[_blockNumber][i]];
            if (order.status == ExitOrderState.Deffered) {
                rollup.trySwiftExitWithdraw(
                    _blockNumber,
                    order.withdrawOpOffset,
                    order.withdrawOpHash,
                    order.recipient
                );
            }
        }
    }

    /// @notice Punishes for failed orders
    /// @dev Reduces validators supplies for failed orders, reduces total borrow
    /// @param _succeededHashes Succeeded orders hashes
    function punishForFailed(uint256[] memory _failedHashes) internal {
         for (uint32 i = 0; i < _failedHashes.length; i++) {
            BorrowOrder bOrder = borrowOrders[_failedHashes[i]];
            borrows[exitOrder.supplyTokenId] -= bOrder.borrowed;
            supplies[exitOrder.supplyTokenId] -= bOrder.borrowed;
        }
    }

    /// @notice Check if the sender is rollup contract
    function requireRollup() internal view {
        require(
            msg.sender == rollupAddress,
            "ssrp11"
        ); // ssrp11 - only by rollup
    }

    /// @notice Check if the sender is owner contract
    function requireOwner() internal view {
        require(
            msg.sender == owner,
            "ssrr21"
        ); // ssrr21 - only by owner
    }
}
