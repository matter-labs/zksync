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

    /// @notice Validators fee coeff (Must be devided by 100)
    uint256 constant VALIDATORS_FEE_COEFF = 5;

    /// @notice Borrowing from validators coeff (borrowing amount = needed amount * BORROWING_COEFF)
    uint256 constant BORROWING_COEFF = 3;

    /// @notice Matter token id
    uint256 internal matterTokenId;

    /// @notice Matter token contract address
    address internal matterTokenAddress;

    /// @notice cMatter token contract address
    address internal cMatterTokenAddress;

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
    struct ExitOrder {
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

        // Set matter token address, id, cToken corresponding address
        matterTokenAddress = _matterTokenAddress;
        matterTokenId = governance.validateTokenAddress(_matterTokenAddress);
        cMatterTokenAddress = governance.getCTokenAddress(matterTokenId);

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
    /// @param _aggrSignatureX Aggregated validators signature x
    /// @param _aggrSignatureY Aggregated validators signature y
    /// @param _signersBitmask Validators-signers bitmask
    function addSwiftExit(
        bytes memory _swiftExit,
        uint256 _aggrSignatureX,
        uint256 _aggrSignatureY,
        uint16 _signersBitmask
    )
        external
    {
        // Swift Exit data:
        // blockNumber Rollup block number
        // onchainOpNumber Withdraw operation number in block
        // accNumber Account - creator of withdraw operation
        // tokenId Token id
        // tokenAmount Token amount
        // feeAmount Fee amount in specified tokens
        // recipient Withdraw operation recipient
        (
            uint32 blockNumber,
            uint64 onchainOpNumber,
            uint24 accNumber,
            uint16 tokenId,
            uint256 tokenAmount,
            uint16 feeAmount,
            address recipient
        ) = parceSwiftExit(_swiftExit);

        // This transaction cost for validator-sender
        uint256 creationCost = getCreationCostForToken(_tokenId);

        // Swift Exit hash
        uint256 swiftExitHash = uint256(keccak256(_swiftExit));

        // Withdraw peration hash
        uint256 opHash = uint256(keccak256(abi.encodePacked(
            accNumber,
            tokenId,
            tokenAmount,
            feeAmount,
            recipient
        )));

        // Checks
        require(
            !exitOrdersExistance[blockNumber][onchainOpNumber],
            "ssat11"
        ); // "ssat11" - order exists
        require(
            governance.verifySenderAndBlsSignature(
                msg.sender,
                _aggrSignatureX,
                _aggrSignatureY,
                _signersBitmask,
                swiftExitHash
            ),
            "ssat12"
        ); // "ssat12" - wrong signature or validator-sender is not in signers bitmask

        // Get last verified block
        uint32 lastVerifiedBlock = rollup.totalBlocksVerified;

        if (blockNumber <= lastVerifiedBlock) {
            // If order block is already verified - try to withdraw funds directly from rollup

            // Check if tokenAmount is higher than creation cost
            require(
                creationCost < tokenAmount,
                "ssat13"
            ); // "ssat14" - tokenAmount is higher than creation cost

            // Sending amount: token amount minus creation cost
            uint256 sendingAmount = tokenAmount - creationCost;

            // Check this withdraw operation existance in block
            uint64 onchainOpsStartIdInBlock = rollup.blocks[_blockNumber].startId;
            uint256 realOpHash = uint256(keccak256(rollup.onchainOps[onchainOpsStartIdInBlock + onchainOpNumber].pubData));
            require(
                realOpHash == opHash,
                "ssat14"
            ); // "ssat14" - expected hash is not equal to real withdraw operation hash

            // Try withdraw from rollup and freeze transaction cost
            rollup.swiftExitWithdraw(
                blockNumber,
                tokenId,
                sendingAmount,
                creationCost,
                recipient
            );

            // Create Exit order with zero values everywhere, except creation cost - to pay for validator-sender
            ExitOrder order = ExitOrder(
                onchainOpNumber,
                opHash,
                tokenId,
                0,
                creationCost,
                0,
                msg.sender,
                _signersBitmask,
                0,
                0,
                0
            );
            exitOrders[_blockNumber][exitOrdersCount[_blockNumber]] = order;
            // Increase orders count in block
            exitOrdersCount[_blockNumber]++;
            // Set order existance
            exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;
        } else  {
            // If order block is not already verified - try to borrow tokens from validators to perform swift exit
           
            // Calculate validators fees: token amount * VALIDATORS_FEE_COEFF / 100
            uint256 validatorsFee = tokenAmount * VALIDATORS_FEE_COEFF / 100;
            
            // Check if tokenAmount is higher than sum of validators fees and transaction cost
            require(
                creationCost + validatorsFee < tokenAmount,
                "ssat15"
            ); // "ssat15" - wrong amount

            // Freeze tokenAmount on rollup
            rollup.freezeFunds(
                _blockNumber,
                tokenId,
                tokenAmount,
                recipient
            );

            // Sending amount: token amount minus sum of validators fees and transaction cost
            uint256 sendingAmount = tokenAmount - (creationCost + validatorsFee);

            // Borrow tokens from validators and exchange with compound if needed (if validators haven't got enouth specified tokens - try to borrow Matter token)
            // Get borrowed from validators token id (supplied to compound if needed), its amount and validators-lenders count
            (
                uint16 supplyTokenId,
                uint256 supplyAmount,
                uint16 suppliersCount
            ) = exchangeTokens(tokenId, sendingAmount);

            // Send tokens to recepient
            sendTokensToRecipient(recipient, tokenId, sendingAmount);

            // Create ExitOrder
            ExitOrder order = ExitOrder(
                onchainOpNumber,
                opHash,
                tokenId,
                sendingAmount,
                creationCost,
                validatorsFee,
                msg.sender,
                _signersBitmask,
                suppliersCount,
                supplyTokenId,
                supplyAmount
            );
            exitOrders[_blockNumber][exitOrdersCount[_blockNumber]] = order;
            // Increase orders count in block
            exitOrdersCount[_blockNumber]++;
            // Set order existance
            exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;
        }
    }

    /// @notice Returns order token id, amount, recipient
    /// @param _blockNumber Block number
    /// @param _orderNumber Order number
    function getOrderInfo(uint32 _blockNumber, uint64 _orderNumber) external returns (uint16 tokenId, uint256 amount, address recipient) {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnds11"
        ); // fnds11 - wrong address
        ExitOrder order = exitOrders[_blockNumber][_orderNumber];
        return (
            order.tokenId,
            order.sendingAmount + order.creationCost + order.validatorsFee,
            order.recipient
        );
    }

    /// @notice Get orders success status for block
    /// @param _blockNumber Block number
    function getOrdersSuccessStatusList(uint32 _blockNumber) external returns (bytes memory succeeded) {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnds11"
        ); // fnds11 - wrong address
        // Requires verified blocks count is higher than specified block number
        require(
            rollup.totalBlocksVerified >= _blockNumber,
            "fnds12"
        ); // fnds12 - wrong block number
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
                // If hashes are equal - add 1
                succeeded[i] = 1;
            } else {
                // If hashes aren't equal - add 0
                succeeded[i] = 0;
            }
        }
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

    /// @notice Borrow tokens from validators and exchange with compound if needed (if validators haven't got enouth specified tokens - try to borrow Matter token)
    /// @dev Returns borrowed from validators token id (supplied to compound if needed), its amount and validators-lenders count
    /// @param _tokenId Token id
    /// @param _sendingAmount Amount that will be sent to recipient
    function exchangeTokens(
        uint16 _tokenId,
        uint256 _sendingAmount
    )
        internal
        returns (
            uint16 supplyTokenId,
            uint256 supplyAmount,
            uint16 suppliersCount
        )
    {
        // Try borrow directly specified token from validators and return similar token id and amount * BORROWING_COEFF
        // Also get validators-suppliers count
        suppliersCount = governance.borrowToTrustedAddress(_tokenId, BORROWING_COEFF * _sendingAmount);
        if (suppliersCount > 0) {
            // If suppliers count is > 0 - validators have enouth funds
            return (_tokenId, BORROWING_COEFF * _sendingAmount, suppliersCount);
        }

        // Borrow Matter token if previous failed

        // Get corresponding cToken address for specified token
        address cTokenAddress = governance.getCTokenAddress(_tokenId);

        // Get token price from oracle
        uint256 tokenPrice = priceOracle.getUnderlyingPrice(cTokenAddress);

        // Get Matter token price from oracle
        uint256 matterTokenPrice = priceOracle.getUnderlyingPrice(cMatterTokenAddress);

        // Get token collateral factor mantissa (0..90%) that will show what part of supplied tokens to compound will be used for borrowing
        // Also returns listed flag (if token is listed on compound markets)
        (bool listed, uint256 collateralFactorMantissa) = comptroller.markets(cTokenAddress);
        require(
            listed,
            "sses11"
        ); // "sses11" - token is not listed on compound markets

        // Matter token amount, that will be sent to compound. Is equal to:
        // sendingAmount * (Matter token price / token price) * (collateralFactorMantissa / 100)
        uint256 matterTokenAmount = _sendingAmount * (matterTokenPrice / tokenPrice) / (collateralFactorMantissa / 100);
        
        // Try borrow Matter token from validators (amount to borrow is: BORROWING_COEFF * matterTokenAmount)
        // Also get validators-suppliers count
        suppliersCount = governance.borrowToTrustedAddress(matterTokenId, BORROWING_COEFF * matterTokenAmount);
        require(
            suppliersCount > 0,
            "sses12"
        ); // "sses12" - if suppliers count is 0 - validators don't have enouth funds

        // Borrow needed tokens for Matter tokens from compound
        borrowFromCompound(matterTokenId, matterTokenAmount, _tokenId, _sendingAmount);

        // Return Matter token id, borrowed Matter tokens value from validators, validators-suppliers count
        return (matterTokenId, BORROWING_COEFF * matterTokenAmount, suppliersCount);
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
    /// @param _blockNumber - Rollup block number
    /// @param _orderNumber - Swift Exit order number
    function fulfillSucceededOrder(uint32 _blockNumber, uint64 _orderNumber) internal {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnds11"
        ); // fnds11 - wrong address

        // Get exit order
        ExitOrder order = exitOrders[_blockNumber][_orderNumber];
        
        // If supplyAmount > 0 - returns borrowed validators funds
        if (order.supplyAmount > 0) {
            if (order.tokenId != order.supplyTokenId) {
                // If order token id is not equal to supplied token id - need to repay borrow to compound
                repayToCompound(
                    order.tokenId,
                    order.sendingAmount,
                    order.supplyTokenId,
                    order.supplyAmount / BORROWING_COEFF
                );
            }

            if (order.supplyTokenId == 0) {
                // If supplied token id is 0 - repay validators in Ether
                governance.repayInEther.value(order.supplyAmount)(order.suppliersCount, 0);
            } else {
                // If supplied token id is not 0 - repay validators in ERC20

                // Validate token id - get token address
                address tokenAddress = governance.validateTokenId(supplyTokenId);

                // Check for enouth allowence value for this token for repay to Governance contract
                uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
                // Allowence must be >= supply amount
                if (allowence < order.supplyAmount) {
                    // If allowence value is not anouth - approve max possible value for this token for repay to Governance contract
                    require(
                        IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                        "ssfr11"
                    ); // ssfr11 - token approve failed
                }

                // Repay to Governance in Ether
                governance.repayInErc20(
                    order.supplyTokenId,
                    order.supplyAmount,
                    v.suppliersCount,
                    0
                );
            }
        }

        // Consummate fees
        if (order.tokenId == 0) {
            // If order token id is 0 - pay validators fee and creation cost in Ether
            governance.repayInEther.value(order.validatorsFee);
            governance.supplyEther.value(order.creationCost)(order.validatorSender);
        } else {
            // If order token id is 0 - pay validators fee and creation cost in ERC20

            // Validate token id - get token address
            address tokenAddress = governance.validateTokenId(order.tokenId);

            // Check for enouth allowence value for this token for pay fees to Governance contract
            uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
            // Allowence must be >= validators fee + creation cost
            if (allowence < order.validatorsFee + order.creationCost) {
                // If allowence value is not anouth - approve max possible value for this token for repay to Governance contract
                require(
                    IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "ssfr11"
                ); // ssfr11 - token approve failed
            }

            // Pay fees
            governance.repayInErc20(
                order.tokenId,
                order.validatorsFee,
                order.suppliersCount,
                0
            );

            // Pay creation cost
            governance.supplyErc20(
                order.tokenId,
                order.creationCost,
                order.validatorSender
            );
        }
    }

    /// @notice Punishes validators-signers for failed order
    /// @dev Repay unsent amount of supply to validators, that haven't signed order
    /// @param _blockNumber - Rollup block number
    /// @param _orderNumber - Swift Exit order number
    function punishForFailedOrder(uint32 _blockNumber, uint64 _orderNumber) internal {
        // Can be called only from Rollup contract
        require(
            msg.sender == address(rollup),
            "fnds11"
        ); // fnds11 - wrong address

        // Get exit order
        ExitOrder order = exitOrders[_blockNumber][_orderNumber];

        // Repay unsent amount of supply to validators, that haven't signed order if supplied amount is > 0
        if (order.supplyAmount > 0) {
            if (order.supplyTokenId == 0) {
                // Repay in Ether if supply token id == 0

                // Repayment value is supplied value minus = supply amount * (borrowing coeff - 1) / borrowing coeff
                uint356 value = order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF;
                // Repay in Ether to validators possible value, excluding (punish) validators-signers
                governance.repayInEther.value(value)(
                    order.suppliersCount,
                    order.signersBitmask
                );
            } else {
                // Repay in ERC20 if supply token id != 0

                // Repayment value is supplied value minus = supply amount * (borrowing coeff - 1) / borrowing coeff
                uint356 value = order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF;
                
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
                    order.supplyTokenId,
                    order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF,
                    order.suppliersCounts,
                    order.signersBitmask
                );
            }
        }
    }
}
