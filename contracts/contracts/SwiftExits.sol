pragma solidity ^0.5.8;

import "./IERC20.sol";
import "./IComptroller.sol";
import "./ICEther.sol";
import "./ICErc20.sol";

import "./Governance.sol";
import "./Franklin.sol";

/// @title Swift Exits Contract
/// @author Matter Labs
contract SwiftExits {

    /// @notice Governance contract
    Governance internal governance;

    /// @notice Rollup contract
    Franklin internal rollup;

    /// @notice Comptroller contract
    Comptroller internal comptroller;

    /// @notice Swift Exits orders by Rollup block number (block number -> order number -> order)
    mapping(uint32 => mapping(uint64 => ExitOrder)) public exitOrders;
    
    /// @notice Swift Exits in blocks count (block number -> orders count)
    mapping(uint32 => uint64) public exitOrdersCount;

    /// @notice Swift Exits existance in block with specified withdraw operation number (block number -> withdraw op number -> existance)
    mapping(uint32 => mapping(uint64 => bool)) public exitOrdersExistance;

    /// @notice Container for information about Swift Exit Order
    /// @member onchainOpNumber Withdraw operation number in block
    /// @member opHash Corresponding onchain operation hash
    /// @member tokenId Order (sending) token id
    /// @member sendingAmount Sending amount to recipient (initial amount minus fees)
    /// @member recipient Recipient of the withdraw operation
    /// @member validatorsFee Fee for validators in orders' tokens
    /// @member validatorSender Address of validator-sender (order transaction sender)
    /// @member supplyAmount Supplied token amount
    struct ExitOrder {
        uint64 onchainOpNumber;
        uint256 opHash;
        uint16 tokenId;
        uint256 sendingAmount;
        address recipient;
        uint256 validatorsFee;
        address validatorSender;
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
    function setupRelatedContracts(
        address _matterTokenAddress,
        address _rollupAddress,
        address _comptrollerAddress
    )
        external
    {
        // Check for governor
        governance.requireGovernor();

        // Set contracts by addresses
        rollup = Franklin(_rollupAddress);
        comptroller = Comptroller(_comptrollerAddress);
    }

    /// @notice Adds new swift exit
    /// @dev Only validator can send this order, validates validators aggregated signature, requires that order must be new
    /// @param _blockNumber Rollup block number
    /// @param _onchainOpNumber Withdraw operation number in block
    /// @param _accNumber Account - creator of withdraw operation
    /// @param _tokenId Token id
    /// @param _tokenAmount Token amount
    /// @param _feeAmount Fee amount in specified tokens
    /// @param _recipient Withdraw operation recipient
    /// @param _owner Withdraw operation owner
    /// @param _swiftExitFee Validators fee
    /// @param _supplyAmount Supplied amount
    function addSwiftExit(
        uint32 _blockNumber,
        uint64 _onchainOpNumber,
        uint24 _accNumber,
        uint16 _tokenId,
        uint256 _tokenAmount,
        uint256 _feeAmount,
        uint256 _recipient,
        uint256 _owner,
        uint256 _swiftExitFee,
        uint256 _supplyAmount
    )
        external
    {
        // Can be called only from Governance contract
        require(
            msg.sender == address(governance),
            "fnds11"
        ); // fnds11 - wrong address
        // Check that order is new
        require(
            !exitOrdersExistance[_blockNumber][_onchainOpNumber],
            "ssat12"
        ); // "ssat12" - order exists

        // Sending amount: token amount minus sum of validators fees for swift exit and transaction cost
        uint256 sendingAmount = _tokenAmount - _swiftExitFee;

        // Check if tokenAmount is higher than sum of validators fees for swift exit and transaction cost
        require(
            sendingAmount > 0,
            "ssat13"
        ); // "ssat13" - wrong amount

        // Withdraw peration hash
        uint256 opHash = uint256(keccak256(abi.encodePacked(
            _accNumber,
            _tokenId,
            _tokenAmount,
            _feeAmount,
            _recipient,
            _owner
        )));
        
        uint16 matterTokenId = governance.matterTokenId;
        if (_tokenId != matterTokenId) {
            // Borrow needed tokens from compound if token ids arent equal
            borrowFromCompound(_supplyAmount, tokenId, sendingAmount);
        }

        // Send tokens to recipient
        sendTokensToRecipient(_recipient, _tokenId, sendingAmount);

        // Create ExitOrder
        ExitOrder order = ExitOrder(
            _onchainOpNumber,
            opHash,
            _tokenId,
            sendingAmount,
            _recipient,
            _swiftExitFee,
            tx.origin, // Here tx origin MUST be only validator address, so we can use it
            _supplyAmount
        );
        exitOrders[_blockNumber][exitOrdersCount[_blockNumber]] = order;
        // Increase orders count in block
        exitOrdersCount[_blockNumber]++;
        // Set order existance
        exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;
    }

    /// @notice Consummates fees for succeeded and punishes for failed orders
    /// @param _blockNumber Block number
    function completeOrders(uint32 _blockNumber) external {
        // Can be called only by active validator
        governance.requireActiveValidator(msg.sender);

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
                // If hashes are equal - order is correct -> get tokens from rollup, repay to compound if needed, repay to validators
                
                // Defrost funds on rollup and get tokens
                rollup.defrostFunds(
                    order.recipient,
                    order.tokenId,
                    order.sendingAmount + order.validatorsFee
                );

                // Repay to compound if needed
                uint16 matterTokenId = governance.matterTokenId;
                if (order.tokenId != matterTokenId) {
                    repayToCompound(
                        order.tokenId,
                        order.sendingAmount,
                        order.supplyAmount
                    );
                }

                // Check for enouth allowence value for matter token to repay borrow
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
                    governance.repayBorrowWithFees.value(order.validatorsFee)(
                        order.supplyAmount,
                        address(0),
                        0,
                        order.validatorSender
                    );
                } else {
                    // Fees in ERC20
                    address tokenAddress = governance.validateTokenId(order.tokenId);

                    // Check for enouth allowence value for this token to charge fees
                    uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
                    // Allowence must be >= validators fee
                    if (allowence < _order.validatorsFee) {
                        // If allowence value is not enouth - approve max possible value for this token for repay to Governance contract
                        require(
                            IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                            "sscs12"
                        ); // ssfr12 - token approve failed
                    }

                    governance.repayBorrowWithFees.value(0)(
                        order.supplyAmount,
                        tokenAddress,
                        order.validatorsFee,
                        order.validatorSender
                    );
                }
            }
        }
        delete exitOrdersCount[_blockNumber];
        delete exitOrdersExistance[_blockNumber];
        delete exitOrders[_blockNumber];
    }

    /// @notice Borrow specified amount from compound
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
        // Get supply token address
        address supplyTokenAddress = governance.matterTokenAddress;
        // Get supply cToken address (corresponding to supply token)
        address cSupplyTokenAddress = governance.cMatterTokenAddress;

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
    /// @param _redeemAmount Amount of supplied tokens
    function repayToCompound(
        uint16 _tokenRepayId,
        uint256 _repayAmount,
        uint256 _redeemAmount
    )
        internal
    {
        // Get repay token address
        address repayTokenAddress = governance.validateTokenId(_tokenRepayId);
        // Get repay cToken address (corresponding to repay token)
        address cRepayTokenAddress = governance.getCTokenAddress(_tokenRepayId);

        // Get redeem cToken address (corresponding to redeem token)
        address cRedeemTokenAddress = governance.cMatterTokenAddress;

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

        // Redeem cErc20
        CErc20 cToken = CErc20(cRedeemTokenAddress);
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
