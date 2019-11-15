pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";
import "compound-protocol/contracts/CEther.sol";
import "compound-protocol/contracts/CErc20.sol";
import "compound-protocol/contracts/Comptrolnler.sol";
import "compound-protocol/contracts/PriceOracle.sol";

import "./Governance.sol";
import "./Franklin.sol";

/// @title Swift Exits Contract
/// @author Matter Labs
contract SwiftExits {

    /// @notice GAS value to complete Swift Exit order creation transaction
    uint256 constant SWIFT_EXIT_TX_CREATION_GAS = 100000;

    /// @notice cEther token contract address
    address internal cEtherAddress;

    /// @notice Governance contract
    Governance internal governance;

    /// @notice Rollup contract
    Franklin internal rollup;

    /// @notice Comptroller contract
    Comptroller internal comptroller;

    /// @notice PriceOracle contract
    PriceOracle internal priceOracle;

    /// @notice Swift Exits orders by Rollup block number (block number -> order number -> order)
    mapping(uint32 => mapping(uint64 => ExitOrder)) internal exitOrders;
    
    /// @notice Swift Exits in blocks count (block number -> orders count)
    mapping(uint32 => uint64) internal exitOrdersCount;

    /// @notice Swift Exits existance in block with specified withdraw operation number (block number -> withdraw op number -> existance)
    mapping(uint32 => mapping(uint64 => bool)) internal exitOrdersExistance;

    /// @notice Container for information about Swift Exit Order
    /// @memner status Order success status (true for success)
    /// @member onchainOpNumber Withdraw operation number in block
    /// @member tokenId Order (sending) token id
    /// @member sendingAmount Sending amount to recepient (initial amount minus fees)
    /// @member creationCost Cost in orders' token of swift exit operation for validator-sender
    /// @member validatorsFee Fee for validators in orders' tokens
    /// @member validatorSender Address of validator-sender (order transaction sender)
    /// @member signersBitmask Order validators-signers bitmask
    /// @member suppliersCount Order validators-suppliers count
    /// @member supplyTokenId Supplied token id
    /// @member supplyAmount Supplied token amount
    /// @member neededSupplyAmount Supplied token amount that is used for compound
    struct ExitOrder {
        bool status;
        uint64 onchainOpNumber;
        uint256 opHash;
        uint16 tokenId;
        uint256 sendingAmount;
        uint256 creationCost;
        uint256 validatorsFee;
        address validatorSender;
        uint16 signersBitmask;
        uint16 suppliersCount;
        uint16 supplyTokenId;
        uint256 supplyAmount;
        uint256 neededSupplyAmount;
    }

    /// @notice Construct swift exits contract
    /// @param _governanceAddress The address of Governance contract
    constructor(address _governanceAddress) public {
        governance = Governance(_governanceAddress);
    }

    /// @notice Add addresses of related contracts
    /// @dev Requires governor
    /// @param _matterTokenAddress The address of Matter token
    /// @param _rollupAddress The address of Rollup contract
    /// @param _comptrollerAddress The address of Comptroller contract
    /// @param _comptrollerAddress The address of PriceOracle contract
    function setupRelatedContracts(
        address _matterTokenAddress,
        address _rollupAddress,
        address _comptrollerAddress,
        address _priceOracleAddress
    )
        external
    {
        // Check for governor
        governance.requireGovernor();

        // Set contracts by addresses
        rollup = Franklin(_rollupAddress);
        comptroller = Comptroller(_comptrollerAddress);
        priceOracle = PriceOracle(_priceOracleAddress);

        // Set cEther address
        cEtherAddress = governance.getCTokenAddress(0);
    }

    /// @notice Fallback function
    /// @dev Accepts ether only from governance, rollup and cEther addresses
    function() external payable {
        if (
            msg.sender != address(governance) ||
            msg.sender != address(rollup) ||
            msg.sender != cEtherAddress
        ) {
            revert("Cant accept from unexpected contract");
        }
    }

    /// @notice Adds new swift exit
    /// @dev Only validator can send this order, validates validators aggregated signature, requires that order must be new
    /// @param _swiftExit Swift exit data
    /// @param _supplyTokenId Supplied token id
    /// @param _supplyAmount Supplied amount
    /// @param _suppliersCount Suppliers count
    /// @param _sender Sender of transaction (validator-sender)
    function addSwiftExit(
        bytes memory _swiftExit,
        uint16 _supplyTokenId,
        uint256 _supplyAmount,
        uint16 _suppliersCount,
        address _sender
    )
        external
    {
        // Can be called only from Governance contract
        require(
            msg.sender == address(governance),
            "fnds11"
        ); // fnds11 - wrong address

        // Swift Exit data:
        // blockNumber Rollup block number
        // onchainOpNumber Withdraw operation number in block
        // accNumber Account - creator of withdraw operation
        // tokenId Token id
        // tokenAmount Token amount
        // feeAmount Fee amount in specified tokens
        // recipient Withdraw operation recipient
        // owner Withdraw operation owner
        // swiftExitFee Validators fee
        (
            uint32 blockNumber,
            uint64 onchainOpNumber,
            uint24 accNumber,
            uint16 tokenId,
            uint256 tokenAmount,
            uint16 feeAmount,
            address recipient,
            address owner,
            uint256 swiftExitFee
        ) = parceSwiftExit(_swiftExit);

        // This transaction cost for validator-sender
        uint256 creationCost = getCreationCostForToken(_tokenId);

        // Withdraw peration hash
        uint256 opHash = uint256(keccak256(abi.encodePacked(
            accNumber,
            tokenId,
            tokenAmount,
            feeAmount,
            recipient,
            owner
        )));

        // Check that order is new
        require(
            !exitOrdersExistance[blockNumber][onchainOpNumber],
            "ssat12"
        ); // "ssat12" - order exists

        // Sending amount: token amount minus sum of validators fees for swift exit and transaction cost
        uint256 sendingAmount = tokenAmount - (creationCost + swiftExitFee);

        // Check if tokenAmount is higher than sum of validators fees for swift exit and transaction cost
        require(
            sendingAmount > 0,
            "ssat13"
        ); // "ssat13" - wrong amount

        // Get amount of supply needed to get sending amount
        uint256 neededSupplyAmount = getNeededAmount(_supplyTokenId, tokenId, sendingAmount);

        // Check if tokenAmount >= than sum of validators fees for swift exit and transaction cost
        require(
            neededSupplyAmount - _supplyAmount >= 0,
            "ssat14"
        ); // "ssat14" - not enouth supplied

        if (_supplyTokenId != tokenId) {
            // Borrow needed tokens from compound if token ids arent equal
            borrowFromCompound(_supplyTokenId, neededSupplyAmount, tokenId, sendingAmount);
        }

        // Send tokens to recepient
        sendTokensToRecipient(recipient, tokenId, sendingAmount);

        // Create ExitOrder
        ExitOrder order = ExitOrder(
            false,
            onchainOpNumber,
            opHash,
            tokenId,
            sendingAmount,
            creationCost,
            swiftExitFee,
            msg.sender,
            _signersBitmask,
            suppliersCount,
            supplyTokenId,
            supplyAmount,
            neededSupplyAmount
        );
        exitOrders[_blockNumber][exitOrdersCount[_blockNumber]] = order;
        // Increase orders count in block
        exitOrdersCount[_blockNumber]++;
        // Set order existance
        exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;
    }

    /// @notice Returns order token id, amount, recipient
    /// @param _blockNumber Block number
    /// @param _orderNumber Order number
    function getOrderInfo(
        uint32 _blockNumber,
        uint64 _orderNumber
    )
        external
        returns (
            bool isSucceeded,
            uint16 tokenId,
            uint256 amount,
            address recipient
        )
    {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnds11"
        ); // fnds11 - wrong address
        ExitOrder order = exitOrders[_blockNumber][_orderNumber];
        
        return (
            order.status,
            order.tokenId,
            order.sendingAmount + order.creationCost + order.validatorsFee,
            order.recipient
        );
    }

    /// @notice Sets orders success status for block
    /// @param _blockNumber Block number
    function setOrdersStatuses(uint32 _blockNumber) external returns (bytes memory succeeded) {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnds11"
        ); // fnds11 - wrong address
        // Get onchain operations start id in this block from Rollup contract
        uint64 onchainOpsStartIdInBlock = rollup.blocks[_blockNumber].startId;
        // Go into loop for all exit orders in block
        for (uint64 i = 0; i < exitOrdersCount[_blockNumber]; i++) {
            // Get exit order by block nubmer and id
            ExitOrder order = exitOrders[_blockNumber][i];
            // Get real onchain operation hash from Rollup contract
            uint256 realOpHash = uint256(keccak256(rollup.onchainOps[onchainOpsStartIdInBlock + order.onchainOpNumber].pubData));
            // Get expected operation hash from exit order
            uint256 expectedOpHash = order.opHash;

            if (realOpHash == expectedOpHash) {
                // If hashes are equal - success
                exitOrders[_blockNumber][i].status = true;
            }
        }
    }

    /// @notice Consummates fees for succeeded and punishes for failed orders
    /// @param _blockNumber Block number
    function fulfillOrders(uint32 _blockNumber) external {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnfs11"
        ); // fnfs11 - wrong address
        for (uint64 i = 0; i < exitOrdersCount[_blockNumber]; i++) {
            if (exitOrders[_blockNumber][i].status) {
                fulfillSucceededOrder(exitOrders[_blockNumber][i]);
            } else {
                punishForFailedOrder(exitOrders[_blockNumber][i]);
            }
        }
        delete exitOrdersCount[_blockNumber];
        delete exitOrdersExistance[_blockNumber];
        delete exitOrders[_blockNumber];
    }

    /// @notice Get the amount of tokens, which after conversion to Ether will be equal to the order creation transaction gas cost
    /// @param _tokenId Token id
    function getCreationCostForToken(uint16 _tokenId) internal returns (uint256) {
        // Get ether gas cost = fixed amount of gas * transaction gas price
        uint256 etherGasCost = SWIFT_EXIT_TX_CREATION_GAS * tx.gasprice;

        // Get corresponding cToken address for specified token
        address cTokenAddress = governance.getCTokenAddress(_tokenId);

        // Get price for ether from price oracle
        uint256 etherUnderlyingPrice = priceOracle.getUnderlyingPrice(cEtherAddress);

        // Get price for token from price oracle
        uint256 tokenUnderlyingPrice = priceOracle.getUnderlyingPrice(cTokenAddress);

        // Cost in token is equal to ether gas cost * (token price / ether price)
        return etherGasCost * (tokenUnderlyingPrice / etherUnderlyingPrice);
    }

    /// @notice Returns needed amount of supplied token to exchange with compound
    /// @param _supplyTokenId Supplied token id
    /// @param _tokenId Token id
    /// @param _sendingAmount Amount that will be sent to recipient
    function getNeededAmount(
        uint16 _supplyTokenId,
        uint16 _tokenId,
        uint256 _sendingAmount
    )
        internal
        returns (uint256)
    {
        // If token ids equal - return needed amount = sendingAmount
        if (_supplyTokenId == _tokenId) {
            return _sendingAmount;
        }

        // Get corresponding cToken address for specified supply token
        address cSupplyTokenAddress = governance.getCTokenAddress(_supplyTokenId);

        // Get corresponding cToken address for specified token
        address cTokenAddress = governance.getCTokenAddress(_tokenId);

        // Get supply token price from oracle
        uint256 supplyTokenPrice = priceOracle.getUnderlyingPrice(cSupplyTokenAddress);

        // Get token price from oracle
        uint256 tokenPrice = priceOracle.getUnderlyingPrice(cTokenAddress);

        // Get token collateral factor mantissa (0..90%) that will show what part of supplied tokens to compound will be used for borrowing
        // Also returns listed flag (if token is listed on compound markets)
        (bool listed, uint256 collateralFactorMantissa) = comptroller.markets(cSupplyTokenAddress);
        require(
            listed,
            "sses11"
        ); // "sses11" - token is not listed on compound markets

        // Supply token amount, that will be sent to compound. Is equal to:
        // sendingAmount * (Supply token price / token price) * (collateralFactorMantissa / 100)
        return _sendingAmount * (supplyTokenPrice / tokenPrice) / (collateralFactorMantissa / 100);
    }

    /// @notice Borrow specified amount from compound
    /// @param _tokenSupplyId Token supply id
    /// @param _supplyAmount Amount of supplied tokens
    /// @param _tokenBorrowId Token borrow id
    /// @param _borrowAmount Amount of tokens to borrow
    function borrowFromCompound(
        uint16 _tokenSupplyId,
        uint256 _supplyAmount,
        uint16 _tokenBorrowId,
        uint256 _borrowAmount
    )
        internal
    {
        // Get supply token address
        address supplyTokenAddress = governance.validateTokenId(_tokenSupplyId);
        // Get supply cToken address (corresponding to supply token)
        address cSupplyTokenAddress = governance.getCTokenAddress(_tokenSupplyId);

        // Get borrow cToken address (corresponding to borrow token)
        address cBorrowTokenAddress = governance.getCTokenAddress(_tokenBorrowId);

        // Enter compound markets for this tokens (in order to supply or borrow in a market, it must be entered first)
        address[] memory ctokens = new address[](2);
        ctokens[0] = cSupplyTokenAddress;
        ctokens[1] = cBorrowTokenAddress;
        uint[] memory errors = comptroller.enterMarkets(ctokens);
        require(
            errors[0] == 0 && errors[1] == 0,
            "sebd11"
        ); // sebd11 - enter markets failed

        if (_tokenSupplyId == 0) {
            // If token supply id is 0 - its Ether

            // Supply (mint) cEther
            CEther cToken = CEther(cSupplyTokenAddress);
            require(
                cToken.mint.value(_supplyAmount)() == 0,
                "sebd12"
            ); // sebd12 - token mint failed
        } else {
            // If token supply id is not 0 - its ERC20 token

            // Check for enouth allowence value for this token for supply to cToken contract
            uint256 allowence = IERC20(supplyTokenAddress).allowence(address(this), address(cSupplyTokenAddress));
            // Allowence must be >= supply amount
            if (allowence < _supplyAmount) {
                // If allowence value is not anouth - approve max possible value for this token for supply to cToken contract
                require(
                    IERC20(supplyTokenAddress).approve(address(cSupplyTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "sebd13"
                ); // sebd13 - token approve failed
            }
            
            // Supply (mint) cErc20
            CErc20 cToken = CErc20(cSupplyTokenAddress);
            require(
                cToken.mint(_supplyAmount) == 0,
                "sebd14"
            ); // sebd14 - token mint failed
        }

        if (_tokenBorrowId == 0) {
            // If token borrow id is 0 - its Ether

            // Borrow cEther
            CEther cToken = CEther(cBorrowTokenAddress);
            require(
                cToken.borrow.value(_borrowAmount)() == 0,
                "ssbd14"
            );  // ssbd14 - token borrow failed
        } else {
            // If token borrow id is not 0 - its ERC20 token

            // Borrow cErc20
            CErc20 cToken = CErc20(cBorrowTokenAddress);
            require(
                cToken.borrow(_borrowAmount) == 0,
                "ssbd15"
            );  // ssbd15 - token borrow failed
        }
    }

    /// @notice Repays specified amount to compound
    /// @param _tokenRepayId Token repay id
    /// @param _repayAmount Amount of tokens to repay
    /// @param _tokenRedeemId Token redeem id
    /// @param _redeemAmount Amount of supplied tokens
    function repayToCompound(
        uint16 _tokenRepayId,
        uint256 _repayAmount,
        uint16 _tokenRedeemId,
        uint256 _redeemAmount
    )
        internal
    {
        // Get repay token address
        address repayTokenAddress = governance.validateTokenId(_tokenRepayId);
        // Get repay cToken address (corresponding to repay token)
        address cRepayTokenAddress = governance.getCTokenAddress(_tokenRepayId);

        // Get redeem cToken address (corresponding to redeem token)
        address cRedeemTokenAddress = governance.getCTokenAddress(_tokenRedeemId);

        if (_tokenRepayId == 0) {
            // If token repay id is 0 - its Ether

            // Repay cEther
            CEther cToken = CEther(cRepayTokenAddress);
            require(
                cToken.repayRepay.value(_repayAmount)() == 0,
                "serd11"
            );  // serd11 - token repay failed
        } else {
            // If token repay id is not 0 - its ERC20 token

            // Check for enouth allowence value for this token for repay to cToken contract
            uint256 allowence = IERC20(repayTokenAddress).allowence(address(this), address(cRepayTokenAddress));
            // Allowence must be >= repay amount
            if (allowence < _repayAmount) {
                // If allowence value is not anouth - approve max possible value for this token for repay to cToken contract
                require(
                    IERC20(repayTokenAddress).approve(address(cRepayTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "serd12"
                );  // serd12 - token approve failed
            }

            // Repay cErc20
            CErc20 cToken = CErc20(cRepayTokenAddress);
            require(
                cToken.repayRepay(_borrowAmount) == 0,
                "serd13"
            );  // serd13 - token repay failed
        }

        if (_tokenRedeemId == 0) {
            // If token redeem id is 0 - its Ether

            // Redeem cEther
            CEther cToken = CEther(cRedeemTokenAddress);
            require(
                cToken.redeemUnderlying(_redeemAmount) == 0,
                "serd14"
            );  // serd14 - token redeem failed
        } else {
            // If token redeem id is not 0 - its ERC20 token

            // Redeem cErc20
            CErc20 cToken = CErc20(cRedeemTokenAddress);
            require(
                cToken.redeemUnderlying(_redeemAmount) == 0,
                "serd15"
            );  // serd15 - token redeem failed
        }
    }

    /// @notice Sends specified amount of token to recipient
    /// @param _recipient Recipient address
    /// @param _tokenId Token id
    /// @param _amount Token amount
    function sendTokensToRecipient(
        address _recipient,
        uint16 _tokenId,
        uint256 _amount
    )
        internal
    {
        if (_tokenId == 0) {
            // If token id == 0 -> transfer ether
            _recipient.transfer(_amount);
        } else {
            // If token id != 0 -> transfer ERC20

            // Validate token id on governance contract - get its address
            address tokenAddress = governance.validateTokenId(_tokenId);
            require(
                IERC20(tokenAddress).transfer(_recipient, _amount),
                "ssst11"
            ); // ssst11 - token transfer failed
        }
    }

    /// @notice Fulfills succeeded order
    /// @dev Repays to compound (if needed) and validators, consummates fees
    /// @param _order - ExitOrder sturcture
    function fulfillSucceededOrder(ExitOrder _order) internal {
        
        if (_order.tokenId != _order.supplyTokenId) {
            // If order token id is not equal to supplied token id - need to repay borrow to compound
            repayToCompound(
                _order.tokenId,
                _order.sendingAmount,
                _order.supplyTokenId,
                _order.neededSupplyAmount
            );
        }

        if (_order.supplyTokenId == 0) {
            // If supplied token id is 0 - repay validators in Ether
            governance.repayInEther.value(_order.supplyAmount)(_order.suppliersCount, 0);
        } else {
            // If supplied token id is not 0 - repay validators in ERC20

            // Validate token id - get token address
            address tokenAddress = governance.validateTokenId(supplyTokenId);

            // Check for enouth allowence value for this token for repay to Governance contract
            uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
            // Allowence must be >= supply amount
            if (allowence < _order.supplyAmount) {
                // If allowence value is not anouth - approve max possible value for this token for repay to Governance contract
                require(
                    IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "ssfr11"
                ); // ssfr11 - token approve failed
            }

            // Repay to Governance in Ether
            governance.repayInErc20(
                _order.supplyTokenId,
                _order.supplyAmount,
                v.suppliersCount,
                0
            );
        }

        // Consummate fees
        if (_order.tokenId == 0) {
            // If order token id is 0 - pay validators fee and creation cost in Ether
            governance.repayInEther.value(_order.validatorsFee);
            governance.supplyEther.value(_order.creationCost)(_order.validatorSender);
        } else {
            // If order token id is 0 - pay validators fee and creation cost in ERC20

            // Validate token id - get token address
            address tokenAddress = governance.validateTokenId(_order.tokenId);

            // Check for enouth allowence value for this token for pay fees to Governance contract
            uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
            // Allowence must be >= validators fee + creation cost
            if (allowence < _order.validatorsFee + _order.creationCost) {
                // If allowence value is not anouth - approve max possible value for this token for repay to Governance contract
                require(
                    IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "ssfr11"
                ); // ssfr11 - token approve failed
            }

            // Pay fees
            governance.repayInErc20(
                _order.tokenId,
                _order.validatorsFee,
                _order.suppliersCount,
                0
            );

            // Pay creation cost
            governance.supplyErc20(
                _order.tokenId,
                _order.creationCost,
                _order.validatorSender
            );
        }
    }

    /// @notice Punishes validators-signers for failed order
    /// @dev Repay unsent amount of supply to validators, that haven't signed order
    /// @param _order - ExitOrder sturcture
    function punishForFailedOrder(ExitOrder _order) internal {
        // Repay unsent amount of supply to validators, that haven't signed order if supplied amount is > 0
        if (_order.supplyAmount > 0) {
            if (_order.supplyTokenId == 0) {
                // Repay in Ether if supply token id == 0

                // Repayment value is supplied value minus = supply amount * (borrowing coeff - 1) / borrowing coeff
                uint356 value = _order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF;
                // Repay in Ether to validators possible value, excluding (punish) validators-signers
                governance.repayInEther.value(value)(
                    _order.suppliersCount,
                    _order.signersBitmask
                );
            } else {
                // Repay in ERC20 if supply token id != 0

                // Repayment value is supplied value minus = supply amount * (borrowing coeff - 1) / borrowing coeff
                uint356 value = _order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF;
                
                // Get token address
                address tokenAddress = governance.validateTokenId(supplyTokenId);
                // Check for enouth allowence value for this token for repay to Governance contract
                uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
                // Allowence must be >= repayment value
                if (allowence < value) {
                    // If allowence value is not anouth - approve max possible value for this token for repay to Governance contract
                    require(
                        IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                        "sspr11"
                    ); // sspr11 - token approve failed
                }

                // Repay in ERC20 to validators possible value, excluding (punish) validators-signers
                governance.repayInErc20(
                    _order.supplyTokenId,
                    _order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF,
                    _order.suppliersCounts,
                    _order.signersBitmask
                );
            }
        }
    }
}
