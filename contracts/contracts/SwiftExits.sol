pragma solidity ^0.5.8;

import "./interfaces/IERC20.sol";
import "./interfaces/ComptrollerInterface.sol";
import "./interfaces/ICEther.sol";
import "./interfaces/ICErc20.sol";

import "./Governance.sol";
import "./Franklin.sol";

/// @title Swift Exits Contract
/// @author Matter Labs
contract SwiftExits {

    /// @notice Governance contract, container for validators and tokens
    Governance internal governance;

    /// @notice Rollup contract, contains user funds
    Franklin internal rollup;

    /// @notice Comptroller contract, allows borrow tokens for validators supply
    ComptrollerInterface internal comptroller;

    /// @notice Validator-creator fee coefficient
    uint256 constant VALIDATOR_CREATOR_FEE_COEFF = 2;

    /// @notice Swift Exits orders by Rollup block number (block number -> order number -> order)
    mapping(uint32 => mapping(uint64 => ExitOrder)) public exitOrders;
    
    /// @notice Swift Exits in blocks count (block number -> orders count)
    mapping(uint32 => uint64) public totalExitOrders;

    /// @notice Swift Exits existance in block with specified withdraw operation number (block number -> withdraw op number -> existance)
    mapping(uint32 => mapping(uint64 => bool)) public exitOrdersExistance;

    /// @notice Container for information about Swift Exit Order
    /// @member onchainOpNumber Withdraw operation number in block
    /// @member opHash Corresponding onchain operation hash
    /// @member tokenId Order (sending/fees) token id
    /// @member sendingAmount Token amount that will be sent to recipient
    /// @member owner Owner of the withdraw operation
    /// @member validatorsFee Fee for validators that signed request in orders' tokens + Fee for the validator that created request
    /// @member validatorSender Address of the validator that created request (order transaction sender)
    /// @member supplyAmount Supplied by validators token amount
    struct ExitOrder {
        uint64 onchainOpNumber;
        uint256 opHash;
        uint16 tokenId;
        uint128 sendingAmount;
        address onwer;
        uint16 validatorsFee;
        address validatorSender;
        uint256 supplyAmount;
    }

    /// @notice Constructs swift exits contract
    /// @param _governanceAddress The address of Governance contract
    constructor(address _governanceAddress) public {
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

        // Check for governor
        governance.requireGovernor();

        // Set contracts by addresses
        rollup = Franklin(_rollupAddress);
        comptroller = ComptrollerInterface(_comptrollerAddress);
    }

    /// @notice Saves a new swift exit. If needed borrows from tokens from compound  for validators supply, sends them to swift exit recipient
    /// @dev Requires validator to be tx.origin and Governance contract to be msg.sender
    /// @param _blockNumber Rollup block number
    /// @param _onchainOpNumber Withdraw operation number in rollup block
    /// @param _accNumber Account - creator of withdraw operation (id in Rollup)
    /// @param _tokenId Token id (sending/fees)
    /// @param _tokenAmount Token amount (sending amount + swift exit fees)
    /// @param _feeAmount Rollup fee amount in specified tokens, used only to create withdraw op hash
    /// @param _packedSwiftExitFee Fee for validators that signed request and validator that sent this tx
    /// @param _owner Withdraw operation owner
    /// @param _recipient Withdraw operation recipient
    /// @param _supplyAmount Supplied amount by validators to borrow from Compound / send to recipient
    function addSwiftExit(
        uint32 _blockNumber,
        uint64 _onchainOpNumber,
        uint24 _accNumber,
        uint16 _tokenId,
        uint128 _tokenAmount,
        uint16 _feeAmount,
        uint16 _packedSwiftExitFee,
        address _owner,
        address _recipient,
        uint256 _supplyAmount
    )
        external
    {
        // Can be called only from Governance contract
        require(
            msg.sender == address(governance),
            "fnds11"
        ); // fnds11 - wrong address

        // Tx origin must be active validator
        require(
            governance.requireActiveValidator(tx.origin),
            "fnds12"
        ); // fnds12 - wrong address

        // Check that order is new
        require(
            !exitOrdersExistance[_blockNumber][_onchainOpNumber],
            "ssat13"
        ); // "ssat13" - order exists

        // Get swift exit fee to reduce token amount by this value
        uint128 swiftExitFee = Bytes.parseFloat(_packedSwiftExitFee);

        // Amount that will be borrowed from Compounds and sent to recipient
        uint128 sendingAmount = _tokenAmount - swiftExitFee;

        // Withdraw operation hash - will be used to check correctness of this request (this hash must be equal to corresponding withdraw op hash on Rollup contract)
        uint256 opHash = uint256(keccak256(abi.encodePacked(
            _accNumber,
            _tokenId,
            _tokenAmount,
            _feeAmount,
            _packedSwiftExitFee,
            _owner,
            _recipient
        )));
        
        // Get matter token id from governance to check if there is need to borrow from Compound (if sending token is also Matter token - no need)
        uint16 matterTokenId = governance.matterTokenId;
        if (_tokenId != matterTokenId) {
            // Borrow needed tokens from compound if token ids arent equal
            borrowFromCompound(_supplyAmount, tokenId, sendingAmount);
        }

        // Send tokens to recipient
        sendTokensToRecipient(_recipient, _tokenId, sendingAmount);

        // Create and save ExitOrder
        ExitOrder order = ExitOrder(
            _onchainOpNumber,
            opHash,
            _tokenId,
            sendingAmount,
            _owner,
            packedSwiftExitFee,
            tx.origin, // Here tx origin MUST be only validator address, so we can use it
            _supplyAmount
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
        uint64 onchainOpsStartIdInBlock = rollup.blocks[_blockNumber].startId;

        // Go into loop for all exit orders in this block
        for (uint64 i = 0; i < totalExitOrders[_blockNumber]; i++) {
            // Get exit order by block nubmer and id
            ExitOrder order = exitOrders[_blockNumber][i];

            // Validator creator fee (she created request)
            uint128 validatorCreatorFee = order.validatorsFee / VALIDATOR_CREATOR_FEE_COEFF;

            // Get real onchain operation hash from Rollup contract
            uint256 realOpHash = uint256(keccak256(rollup.onchainOps[onchainOpsStartIdInBlock + order.onchainOpNumber].pubData));

            // Get expected operation hash from exit order
            uint256 expectedOpHash = order.opHash;

            if (realOpHash == expectedOpHash) {
                // If hashes are equal - order is correct -> get tokens from rollup, repay to compound if needed, repay to validators

                // Defrost funds on rollup, pay fee to validator-creator and get tokens from rollup to repay borrowed from validators
                rollup.defrostFunds(
                    onchainOpsStartIdInBlock + order.onchainOpNumber,
                    order.owner,
                    order.tokenId,
                    order.sendingAmount + order.validatorsFee,
                    validatorCreatorFee,
                    order.validatorSender,
                    true
                );

                // Repay to compound if order token id is not Matter token
                uint16 matterTokenId = governance.matterTokenId;
                if (order.tokenId != matterTokenId) {
                    repayToCompound(
                        order.tokenId,
                        order.sendingAmount,
                        order.supplyAmount
                    );
                }

                // Check for enouth allowence value for Matter token to repay borrow to validators
                address matterTokenAddress = governance.matterTokenAddress;
                uint256 allowence = IERC20(matterTokenAddress).allowence(address(this), address(governance));
                // Allowence must be >= supplied amount
                if (allowence < _order.supplyAmount) {
                    // If allowence value is not enouth - approve max possible value for this token for repay to Governance contract
                    require(
                        IERC20(matterTokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                        "sscs11"
                    ); // ssfr11 - token approve failed
                }
                
                // Repay borrows and charge fees to validators
                if (order.tokenId == 0) {
                    // Fees in Ether
                    governance.repayBorrowWithFees.value(order.validatorsFee - validatorCreatorFee)(
                        order.supplyAmount,
                        address(0),
                        0
                    );
                } else {
                    // Fees in ERC20
                    address tokenAddress = governance.validateTokenId(order.tokenId);

                    // Check for enouth allowence value for this token to charge fees
                    uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
                    // Allowence must be >= validators fee
                    if (allowence < order.validatorsFee - validatorCreatorFee) {
                        // If allowence value is not enouth - approve max possible value for this token for repay to Governance contract
                        require(
                            IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                            "sscs12"
                        ); // ssfr12 - token approve failed
                    }

                    governance.repayBorrowWithFees.value(0)(
                        order.supplyAmount,
                        tokenAddress,
                        order.validatorsFee - validatorCreatorFee
                    );
                }
            } else {
                // If hashes aren't equal - just defrost user funds
                rollup.defrostFunds(
                    onchainOpsStartIdInBlock + order.onchainOpNumber,
                    order.owner,
                    order.tokenId,
                    order.sendingAmount + order.validatorsFee,
                    0,
                    address(0),
                    false
                );
            }
        }
        // Delete storage records for specified block number
        delete totalExitOrders[_blockNumber];
        delete exitOrdersExistance[_blockNumber];
        delete exitOrders[_blockNumber];
    }

    /// @notice Borrow specified token amount from Compound for Matter token
    /// @param _supplyAmount Amount of supplied tokens
    /// @param _tokenBorrowId Token borrow id
    /// @param _borrowAmount Amount of tokens to borrow
    function borrowFromCompound(
        uint256 _supplyAmount,
        uint16 _tokenBorrowId,
        uint256 _borrowAmount
    )
        internal
    {
        // Get Matter token address
        address matterTokenAddress = governance.matterTokenAddress;
        // Get cMatter token address (corresponding to Matter token on Compound)
        address cMatterTokenAddress = governance.cMatterTokenAddress;

        // Get borrow cToken address (corresponding to borrow token on Compound)
        address cBorrowTokenAddress = governance.getCTokenAddress(_tokenBorrowId);

        // Enter compound markets for this tokens (in order to supply or borrow in a market, it must be entered first)
        address[] memory ctokens = new address[](2);
        ctokens[0] = cMatterTokenAddress;
        ctokens[1] = cBorrowTokenAddress;
        uint[] memory errors = comptroller.enterMarkets(ctokens);
        require(
            errors[0] == 0 && errors[1] == 0,
            "sebd11"
        ); // sebd11 - enter markets failed

        // Check for enouth allowence value for this token for supply to cMatter contract
        uint256 allowence = IERC20(matterTokenAddress).allowence(address(this), address(cMatterTokenAddress));
        // Allowence must be >= supply amount
        if (allowence < _supplyAmount) {
            // If allowence value is not anouth - approve max possible value for this token for supply to cMatter contract
            require(
                IERC20(matterTokenAddress).approve(address(cMatterTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                "sebd12"
            ); // sebd12 - token approve failed
        }
        
        // Supply (mint) cErc20
        CErc20 cToken = CErc20(cMatterTokenAddress);
        require(
            cToken.mint(_supplyAmount) == 0,
            "sebd13"
        ); // sebd13 - token mint failed

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

    /// @notice Repays specified amount to compound and redeems Matter tokens
    /// @param _tokenRepayId Token repay id
    /// @param _repayAmount Amount of tokens to repay
    /// @param _redeemAmount Amount of supplied Matter tokens
    function repayToCompound(
        uint16 _tokenRepayId,
        uint256 _repayAmount,
        uint256 _redeemAmount
    )
        internal
    {
        // Get repay token address
        address repayTokenAddress = governance.validateTokenId(_tokenRepayId);
        // Get repay cToken address (corresponding to repay token on Compound)
        address cRepayTokenAddress = governance.getCTokenAddress(_tokenRepayId);

        // Get redeem cMatteraddress (corresponding to Matter token on Compound)
        address cMatterTokenAddress = governance.cMatterTokenAddress;

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

        // Redeem cMatter
        CErc20 cToken = CErc20(cMatterTokenAddress);
        require(
            cToken.redeemUnderlying(_redeemAmount) == 0,
            "serd14"
        );  // serd14 - token redeem failed
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
}
