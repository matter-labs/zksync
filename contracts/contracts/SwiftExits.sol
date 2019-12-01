pragma solidity ^0.5.8;

import "./IERC20.sol";
import "./ComptrollerInterface.sol";
import "./ICEther.sol";
import "./ICErc20.sol";
import "./Bytes.sol";

import "./Governance.sol";
import "./Franklin.sol";

/// @title Swift Exits Contract
/// @author Matter Labs
contract SwiftExits {

    // Constants that will be used in swift exit bytes request parsing

    /// @notice Block number bytes length
    uint8 constant BLOCK_NUMBER_LEN = 4;

    /// @notice Onchain operation number bytes length
    uint8 constant ONCHAIN_OP_NUMBER_LEN = 8;

    /// @notice Account id bytes length
    uint8 constant ACC_ID_LEN = 3;

    /// @notice Token id bytes length
    uint8 constant TOKEN_ID_LEN = 2;

    /// @notice Amount bytes length
    uint8 constant AMOUNT_LEN = 16;

    /// @notice Packed amount bytes length
    uint8 constant PACKED_AMOUNT_LEN = 2;

    /// @notice Address bytes length
    uint8 constant ADDRESS_LEN = 20;

    /// @notice Full amount bytes length (uint256)
    uint8 constant FULL_AMOUNT_LEN = 32;

    /// @notice Governance contract, container for validators and tokens
    Governance internal governance;

    /// @notice Rollup contract, contains user funds
    Franklin internal rollup;

    /// @notice Comptroller contract, allows borrow tokens for validators supply
    ComptrollerInterface internal comptroller;

    /// @notice Validator-creator fee coefficient. Validator that created swift exit reqeust will get as his fee full swift exit fee divided by that coefficient
    uint128 constant VALIDATOR_CREATOR_FEE_COEFF = 2;

    /// @notice Swift Exits orders by Rollup block number (block number -> order number -> order)
    mapping(uint32 => mapping(uint64 => ExitOrder)) public exitOrders;
    
    /// @notice Swift Exits in Rollup blocks count (block number -> orders count)
    mapping(uint32 => uint64) public totalExitOrders;

    /// @notice Swift Exits existance in block with specified withdraw operation number (block number -> withdraw op number -> existance)
    mapping(uint32 => mapping(uint64 => bool)) public exitOrdersExistance;

    /// @notice Container for information about Swift Exit Order
    /// @member opHash Corresponding onchain operation hash
    /// @member supplyAmount Supplied by validators token amount
    /// @member tokenAmount Token amount that will be taken from user (sending amount + validators fee)
    /// @member sendingAmount Token amount that will be sent to recipient
    /// @member onchainOpNumber Withdraw operation number in block
    /// @member tokenId Order (sending/fees) token id
    /// @member owner Owner of the withdraw operation
    /// @member orderCreator Address of the validator that created request (order transaction sender)
    struct ExitOrder {
        uint256 opHash;
        uint256 supplyAmount;
        uint128 tokenAmount;
        uint128 sendingAmount;
        uint64 onchainOpNumber;
        uint16 tokenId;
        address owner;
        address orderCreator;
    }

    /// @notice Constructs swift exits contract
    /// @param _governanceAddress The address of Governance contract
    constructor(address payable _governanceAddress) public {
        governance = Governance(_governanceAddress);
    }

    /// @notice Adds addresses of related contracts
    /// @dev Requires the governor to be msg.sender
    /// @param _rollupAddress The address of Rollup contract
    /// @param _comptrollerAddress The address of Comptroller contract
    function setupRelatedContracts(
        address _rollupAddress,
        address _comptrollerAddress
    )
        external
    {
        require(
            _rollupAddress == address(0) &&
            _comptrollerAddress == address(0),
            "ssss11"
        ); // ssss11 - contracts must be setted only once

        // Check for governor - only governor is allowed to set related contracts
        governance.requireGovernor();

        // Set contracts by their addresses
        rollup = Franklin(_rollupAddress);
        comptroller = ComptrollerInterface(_comptrollerAddress);
    }

    /// @notice Processes a new swift exit: if needed borrows tokens from compound for validators supply, sends them to swift exit recipient, saves exit order in storage
    /// @dev Requires validator to be tx.origin and Governance contract to be msg.sender
    /// @param _opHash Withdraw op hash
    /// @param _supplyAmount Supplied Matter token amount by validators
    /// @param _tokenAmount Token amount (sending amount + swift exit fees)
    /// @param _sendingAmount Token amount for sending to user
    /// @param _onchainOpNumber Withdraw operation number in rollup block
    /// @param _blockNumber Rollup block number
    /// @param _tokenId Token id (sending/fees)
    /// @param _owner Withdraw operation owner
    /// @param _recipient Withdraw operation recipient
    function newSwiftExit(
        uint256 _opHash,
        uint256 _supplyAmount,
        uint128 _tokenAmount,
        uint128 _sendingAmount,
        uint64 _onchainOpNumber,
        uint32 _blockNumber,
        uint16 _tokenId,
        address _owner,
        address payable _recipient
    )
        external
    {
        // Can be called only from Governance contract
        require(
            msg.sender == address(governance),
            "fnds11"
        ); // fnds11 - wrong address

        // Tx origin must be active validator
        governance.requireActiveValidator(tx.origin); // We can use tx.origin because sender (active validator) is actually MUST be tx origin

        // Check that order is new
        require(
            !exitOrdersExistance[_blockNumber][_onchainOpNumber],
            "ssat12"
        ); // "ssat12" - order exists
        
        // Get matter token id from governance to check if there is need to borrow from Compound (if sending token is also Matter token - no need)
        // Token amount to borrow is equal to specified token amount by user minus swift exit fee amount
        // Same amount will be sent to user
        if (_tokenId != governance.matterTokenId()) {
            // Borrow needed tokens from compound if token ids arent equal
            borrowFromCompound(
                _supplyAmount,
                _sendingAmount,
                _tokenId
            );
        }
        // Send tokens to recipient
        sendTokensToRecipient(
            _sendingAmount,
            _recipient,
            _tokenId
        );

        // Create exit order
        createExitOrder(
            _opHash,
            _supplyAmount,
            _tokenAmount,
            _sendingAmount,
            _onchainOpNumber,
            _blockNumber,
            _tokenId,
            _owner
        );
    }

    /// @notice Saves exit order in storage, increases total orders number and saves its existance
    /// @param _opHash Onchain opeartion keccak256 hash
    /// @param _supplyAmount Supplied amount by validators to borrow from Compound / send to recipient
    /// @param _tokenAmount Swift exit token amount (sending amount + swift exit fees)
    /// @param _sendingAmount Token amount that will be sent to recipient
    /// @param _onchainOpNumber Withdraw operation number in rollup block
    /// @param _blockNumber Rollup block number
    /// @param _tokenId Token id (sending/fees)
    /// @param _owner User-owner of corresponding withdraw operation
    function createExitOrder(
        uint256 _opHash,
        uint256 _supplyAmount,
        uint128 _tokenAmount,
        uint128 _sendingAmount,
        uint64 _onchainOpNumber,
        uint32 _blockNumber,
        uint16 _tokenId,
        address _owner
    ) internal {
        // Create and save ExitOrder
        ExitOrder memory order = ExitOrder(
            _opHash,
            _supplyAmount,
            _tokenAmount,
            _sendingAmount,
            _onchainOpNumber,
            _tokenId,
            _owner,
            tx.origin // Here tx origin MUST be only validator address, so we can use it
        );
        exitOrders[_blockNumber][totalExitOrders[_blockNumber]] = order;
        // Increase orders count in block
        totalExitOrders[_blockNumber]++;
        // Set order existance
        exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;
    }

    /// @notice Closes (deletes) Swift Exit Orders for specified Rollup block number
    /// @dev Defrosts user tokens on Rollup contract.
    /// @dev If order is correct (similar withdraw op hashes on it and on Rollup contract) -
    /// @dev repays borrow to compound and repays redeemed Matter tokens and fees to validators on Governance contract
    /// @param _blockNumber Rollup block number in which it is needed to delete orders
    function closeExitOrders(uint32 _blockNumber) external {
        // Can be called only by active validator
        governance.requireActiveValidator(msg.sender);

        // Get onchain operations start id in this block from Rollup contract
        (,,uint64 operationStartId,,,,) = rollup.blocks(_blockNumber);

        // Go into loop for all exit orders in this block
        for (uint64 i = 0; i < totalExitOrders[_blockNumber]; i++) {
            // Get exit order by block nubmer and id
            ExitOrder memory order = exitOrders[_blockNumber][i];

            // Get real onchain operation hash from Rollup contract
            (, bytes memory pubData) = rollup.onchainOps(operationStartId + order.onchainOpNumber);
            uint256 realOpHash = uint256(keccak256(pubData));

            // Get expected operation hash from exit order
            uint256 expectedOpHash = order.opHash;

            if (realOpHash == expectedOpHash) {
                // If hashes are equal - order is correct -> repay borrow to validators
                repaySucceededExitOrder(order, operationStartId);
            } else {
                // If hashes aren't equal - just defrost user funds
                rollup.defrostFunds(
                    order.tokenAmount,
                    0,
                    order.owner,
                    address(0),
                    operationStartId + order.onchainOpNumber,
                    order.tokenId,
                    false
                );
            }
            // Delete storage records for existance and orders
            delete exitOrdersExistance[_blockNumber][order.onchainOpNumber];
            delete exitOrders[_blockNumber][i];
        }
        // Delete storage records for orders number in block
        delete totalExitOrders[_blockNumber];
    }

    /// @notice Defrosts user tokens on Rollup contract. Repays borrow to compound and repays redeemed Matter tokens and fees to validators on Governance contract
    /// @param _order Selected exit order
    /// @param _operationStartId First operation id for rollup block
    function repaySucceededExitOrder(
        ExitOrder memory _order,
        uint64 _operationStartId
    ) internal {
        // Swift exit fee
        uint128 swiftExitFee = _order.tokenAmount - _order.sendingAmount;
        // Validator creator fee (she created request)
        uint128 validatorCreatorFee = swiftExitFee / VALIDATOR_CREATOR_FEE_COEFF;
        // Validators fee
        uint128 validatorsFee = swiftExitFee - validatorCreatorFee;

        // Defrost funds on rollup, pay fee to validator-creator and get tokens from rollup to repay borrowed from validators
        rollup.defrostFunds(
            _order.tokenAmount,
            validatorCreatorFee,
            _order.owner,
            _order.orderCreator,
            _operationStartId + _order.onchainOpNumber,
            _order.tokenId,
            true
        );

        // Repay to compound if order token id is not Matter token
        uint16 matterTokenId = governance.matterTokenId();
        if (_order.tokenId != matterTokenId) {
            repayToCompound(
                _order.sendingAmount,
                _order.supplyAmount,
                _order.tokenId
            );
        }

        // Check for enouth allowance value for Matter token to repay borrow to validators
        address matterTokenAddress = governance.matterTokenAddress();
        uint256 allowance = IERC20(matterTokenAddress).allowance(address(this), address(governance));
        // allowance must be >= supplied amount
        if (allowance < _order.supplyAmount) {
            // If allowance value is not enouth - approve max possible value for this token for repay to Governance contract
            require(
                IERC20(matterTokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                "sscs11"
            ); // ssfr11 - token approve failed
        }
        
        // Repay borrows and charge fees to validators
        if (_order.tokenId == 0) {
            // Fees in Ether

            // Repay borrowed supply with fees to validators
            governance.repayBorrowWithFees.value(validatorsFee)(
                _order.supplyAmount,
                0,
                address(0)
            );
        } else {
            // Fees in ERC20
            address tokenAddress = governance.validateTokenId(_order.tokenId);

            // Check for enouth allowance value for this token to charge fees
            uint256 tokenAllowance = IERC20(tokenAddress).allowance(address(this), address(governance));
            // allowance must be >= validators fee
            if (tokenAllowance < validatorsFee) {
                // If allowance value is not enouth - approve max possible value for this token for repay to Governance contract
                require(
                    IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "sscs12"
                ); // ssfr12 - token approve failed
            }

            // Repay borrowed supply with fees to validators
            governance.repayBorrowWithFees.value(0)(
                _order.supplyAmount,
                validatorsFee,
                tokenAddress
            );
        }
    }

    /// @notice Borrow specified token amount from Compound for Matter token
    /// @param _supplyAmount Amount of supplied tokens
    /// @param _borrowAmount Amount of tokens to borrow
    /// @param _tokenBorrowId Token borrow id
    function borrowFromCompound(
        uint256 _supplyAmount,
        uint256 _borrowAmount,
        uint16 _tokenBorrowId
    )
        internal
    {
        // Get Matter token address
        address matterTokenAddress = governance.matterTokenAddress();
        // Get cMatter token address (corresponding to Matter token on Compound)
        address cMatterTokenAddress = governance.cMatterTokenAddress();

        // Enter compound markets for this tokens (in order to supply or borrow in a market, it must be entered first)
        address[] memory cMatterToken = new address[](1);
        cMatterToken[0] = cMatterTokenAddress;
        uint[] memory errors = comptroller.enterMarkets(cMatterToken);
        require(
            errors[0] == 0,
            "sebd11"
        ); // sebd11 - enter markets failed

        // Check for enouth allowance value for this token for supply to cMatter contract
        uint256 allowance = IERC20(matterTokenAddress).allowance(address(this), address(cMatterTokenAddress));
        // allowance must be >= supply amount
        if (allowance < _supplyAmount) {
            // If allowance value is not anouth - approve max possible value for this token for supply to cMatter contract
            require(
                IERC20(matterTokenAddress).approve(address(cMatterTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                "sebd12"
            ); // sebd12 - token approve failed
        }
        
        // Supply (mint) cErc20
        ICErc20 cTokenSupply = ICErc20(cMatterTokenAddress);
        require(
            cTokenSupply.mint(_supplyAmount) == 0,
            "sebd13"
        ); // sebd13 - token mint failed

        if (_tokenBorrowId == 0) {
            // If token borrow id is 0 - its Ether

            // Get borrow cEther address
            address payable cBorrowTokenAddress = governance.cEtherAddress();

            // Enter compound markets for this tokens (in order to supply or borrow in a market, it must be entered first)
            address[] memory ctokens = new address[](1);
            ctokens[0] = cBorrowTokenAddress;
            uint[] memory errorsE = comptroller.enterMarkets(ctokens);
            require(
                errorsE[0] == 0,
                "sebd14"
            ); // sebd14 - enter markets failed

            // Borrow cEther
            ICEther cToken = ICEther(cBorrowTokenAddress);
            require(
                cToken.borrow(_borrowAmount) == 0,
                "ssbd15"
            );  // ssbd15 - token borrow failed
        } else {
            // If token borrow id is not 0 - its ERC20 token

            // Get borrow cToken address
            address cBorrowTokenAddress = governance.cTokenAddresses(_tokenBorrowId);

            // Enter compound markets for this tokens (in order to supply or borrow in a market, it must be entered first)
            address[] memory ctokens = new address[](1);
            ctokens[0] = cBorrowTokenAddress;
            uint[] memory errorsT = comptroller.enterMarkets(ctokens);
            require(
                errorsT[0] == 0,
                "sebd16"
            ); // sebd16 - enter markets failed

            // Borrow cErc20
            ICErc20 cToken = ICErc20(cBorrowTokenAddress);
            require(
                cToken.borrow(_borrowAmount) == 0,
                "ssbd17"
            );  // ssbd17 - token borrow failed
        }
    }

    /// @notice Repays specified amount to compound and redeems Matter tokens
    /// @param _repayAmount Amount of tokens to repay
    /// @param _redeemAmount Amount of supplied Matter tokens
    /// @param _tokenRepayId Token repay id
    function repayToCompound(
        uint256 _repayAmount,
        uint256 _redeemAmount,
        uint16 _tokenRepayId
    )
        internal
    {
        // Get redeem cMatteraddress (corresponding to Matter token on Compound)
        address cMatterTokenAddress = governance.cMatterTokenAddress();

        if (_tokenRepayId == 0) {
            // If token repay id is 0 - its Ether

            // Get repay cEther address
            address payable cRepayTokenAddress = governance.cEtherAddress();

            // Repay cEther
            ICEther cToken = ICEther(cRepayTokenAddress);
            cToken.repayBorrow.value(_repayAmount)();
        } else {
            // If token repay id is not 0 - its ERC20 token

            // Get repay token address
            address repayTokenAddress = governance.validateTokenId(_tokenRepayId);
            // Get repay cToken address
            address cRepayTokenAddress = governance.cTokenAddresses(_tokenRepayId);

            // Check for enouth allowance value for this token for repay to cToken contract
            uint256 allowance = IERC20(repayTokenAddress).allowance(address(this), address(cRepayTokenAddress));
            // allowance must be >= repay amount
            if (allowance < _repayAmount) {
                // If allowance value is not anouth - approve max possible value for this token for repay to cToken contract
                require(
                    IERC20(repayTokenAddress).approve(address(cRepayTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "serd11"
                );  // serd11 - token approve failed
            }

            // Repay cErc20
            ICErc20 cToken = ICErc20(cRepayTokenAddress);
            cToken.repayBorrow(_redeemAmount);
        }

        // Redeem cMatter
        ICErc20 cToken = ICErc20(cMatterTokenAddress);
        require(
            cToken.redeemUnderlying(_redeemAmount) == 0,
            "serd12"
        );  // serd12 - token redeem failed
    }

    /// @notice Sends specified amount of token to recipient
    /// @param _amount Token amount
    /// @param _recipient Recipient address
    /// @param _tokenId Token id
    function sendTokensToRecipient(
        uint256 _amount,
        address payable _recipient,
        uint16 _tokenId
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
}
