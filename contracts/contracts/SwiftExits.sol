pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";
import "compound-protocol/contracts/CEther.sol";
import "compound-protocol/contracts/CErc20.sol";
import "compound-protocol/contracts/Unitroller.sol";

import "./Governance.sol";
import "./Franklin.sol";

/// @title Swift Exits Contract
/// @notice Consensus version
/// @author Matter Labs
contract SwiftExits is BlsVerifier {

    /// @notice Validators fee coeff
    uint256 constant VALIDATORS_FEE_COEFF = 5;

    /// @notice Borrowing from validators coeff
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
    Unitroller internal comptroller;

    /// @notice Swift Exits hashes (withdraw op hash) by Rollup block number (block number -> order number -> order)
    mapping(uint32 => mapping(uint64 => ExitOrder)) internal exitOrders;
    /// @notice Swift Exits in blocks count (block number -> orders count)
    mapping(uint32 => uint64) internal exitOrdersCount;
    /// @notice Swift Exits existance in block with specified operation number (block number -> op number -> existance)
    mapping(uint32 => mapping(uint64 => bool)) internal exitOrdersExistance;

    /// @notice Container for information about Swift Exit Order
    /// @member onchainOpNumber Withdraw operation offset in block
    /// @member tokenId Order token id
    /// @member initTokenAmount Initial token amount
    /// @member borrowAmount Initial token amount minus fees
    /// @member supplyAmount Order supply amount (in Matter tokens)
    /// @member recipient Recipient address
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
    constructor(address _governanceAddress)
    public
    {
        governance = Governance(_governanceAddress);
    }

    /// @notice Add addresses of related contracts
    /// @dev Requires governor
    /// @param _matterTokenAddress The address of Matter token
    /// @param _rollupAddress The address of Rollup contract
    /// @param _comptrollerAddress The address of Comptroller contract
    function setupRelatedContracts(address _matterTokenAddress,
                                   address _rollupAddress,
                                   address _comptrollerAddress)
    external
    {
        governance.requireGovernor();

        rollup = Franklin(_rollupAddress);
        comptroller = Unitroller(_comptrollerAddress);

        matterTokenAddress = _matterTokenAddress;
        matterTokenId = governance.validateTokenAddress(_matterTokenAddress);
        cMatterTokenAddress = governance.getCTokenAddress(matterTokenId);

        cEtherAddress = governance.getCTokenAddress(0);
    }

    /// @notice Fallback function
    function()
    external
    payable
    {
        if (msg.sender != address(governance) ||
            msg.sender != address(rollup) ||
            msg.sender != cEtherAddress) {
            revert("Cant accept from unexpected contract");
        }
    }

    /// @notice Adds new swift exit
    /// @dev Only governor can send this order, validates validators signature, requires that order must be new (status must be None)
    /// @param _blockNumber Rollup block number
    /// @param _onchainOpNumber Withdraw operation offset in block
    /// @param _withdrawOpHash Withdraw operation hash
    /// @param _tokenId Token id
    /// @param _tokenAmount Token amount
    /// @param _recipient Swift exit recipient
    /// @param _aggrSignatureX Aggregated validators signature x
    /// @param _aggrSignatureY Aggregated validators signature y
    /// @param _signersBitmask Validators-signers bitmask
    function addSwiftExit(bytes memory _swiftExit,
                          uint256 _aggrSignatureX,
                          uint256 _aggrSignatureY,
                          uint16 _signersBitmask)
    external
    {
        // Swift Exit data
        (uint32 blockNumber,
        uint64 onchainOpNumber,
        uint24 accNumber,
        uint16 tokenId,
        uint256 tokenAmount,
        uint16 feeAmount,
        address recipient) = parceSwiftExit(_swiftExit);

        // Swift Exit creation cost
        uint256 creationCost = getCreationCostForToken(_tokenId);

        // Swift Exit hash
        uint256 swiftExitHash = uint256(keccak256(_swiftExit));

        // Operation hash
        uint256 opHash = uint256(keccak256(abi.encodePacked(accNumber,
                                                            tokenId,
                                                            tokenAmount,
                                                            feeAmount,
                                                            recipient)));

        // Checks
        require(
            !exitOrdersExistance[blockNumber][onchainOpNumber],
            "ssat13"
        ); // "ssat13" - request exists
        bool signResult = governance.verifySenderAndBlsSignature(
            msg.sender,
            _aggrSignatureX,
            _aggrSignatureY,
            _signersBitmask,
            swiftExitHash
        );
        require(
            signResult,
            "ssat12"
        ); // "ssat12" - wrong signature

        // get last verified block
        uint32 lastVerifiedBlock = rollup.totalBlocksVerified;

        if (blockNumber <= lastVerifiedBlock) {

            // Check if tokenAmount is higher than sum of fees
            require(
                creationCost < tokenAmount,
                "ssat12"
            ); // "ssat12" - wrong amount

            // Sending amount
            uint256 sendingAmount = tokenAmount - creationCost;

            // Check existance
            uint64 onchainOpsStartIdInBlock = rollup.blocks[_blockNumber].startId;
            uint256 realOpHash = uint256(keccak256(rollup.onchainOps[onchainOpsStartIdInBlock + onchainOpNumber].pubData));
            require(
                realOpHash == opHash,
                "ssat12"
            ); // "ssat12" - wrong amount

            // Try withdraw from rollup
            rollup.withdrawFunds(
                tokenId,
                sendingAmount,
                recipient
            );

            // Freeze funds on rollup
            rollup.freezeFunds(
                tokenId,
                creationCost,
                recipient
            );

            // Create Exit order
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
            exitOrdersCount[_blockNumber]++;
            exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;

        } else  {
            // Get fees
            uint256 validatorsFee = tokenAmount * VALIDATORS_FEE_COEFF / 100;
            
            // Check if tokenAmount is higher than sum of fees
            require(
                creationCost + validatorsFee < tokenAmount,
                "ssat12"
            ); // "ssat12" - wrong amount

            // Freeze funds on rollup
            rollup.freezeFunds(
                tokenId,
                tokenAmount,
                recipient
            );

            // Sending amount
            uint256 sendingAmount = tokenAmount - (creationCost + validatorsFee);

            // // Lock amount
            // uint256 lockAmount = 3 * sendingAmount;

            // Borrow from validators and exchange with compound if needed
            (uint16 supplyTokenId, uint256 supplyAmount, uint16 suppliersCount) = exchangeTokens(tokenId, sendingAmount);

            // Send to recepient
            sendTokensToRecipient(recipient, tokenId, sendingAmount);

            // Create Exit order
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
            exitOrdersCount[_blockNumber]++;
            exitOrdersExistance[_blockNumber][_onchainOpNumber] = true;
        }
    }

    function getCreationCostForToken(uint16 _tokenId)
    internal
    returns (uint256)
    {
        uint256 etherGasCost = SWIFT_EXIT_CREATION_GAS * tx.gasprice; // NEED TO GET FIXED GAS PRICE

        address cTokenAddress = governance.getCTokenAddress(_tokenId);

        uint256 etherUnderlyingPrice = priceOracle.getUnderlyingPrice(cEtherAddress);
        uint256 tokenUnderlyingPrice = priceOracle.getUnderlyingPrice(cTokenAddress);

        return etherGasCost * (tokenUnderlyingPrice / etherUnderlyingPrice);
    }

    function exchangeTokens(uint16 _tokenId,
                            uint256 _sendingAmount)
    internal
    returns (uint16 supplyTokenId,
             uint256 supplyAmount,
             uint16 suppliersCount)
    {
        // try borrow directly specified token from validators
        suppliersCount = governance.borrowToTrustedAddress(_tokenId, BORROWING_COEFF * _sendingAmount);
        if (suppliersCount > 0) {
            return (_tokenId, BORROWING_COEFF * _sendingAmount);
        }

        // borrow matter token if previous failed
        address cTokenAddress = governance.getCTokenAddress(_tokenId);

        uint256 tokenPrice = priceOracle.getUnderlyingPrice(cTokenAddress);
        uint256 matterTokenPrice = priceOracle.getUnderlyingPrice(cMatterTokenAddress);

        (bool listed, uint256 collateralFactorMantissa) = comptroller.markets(cTokenAddress);
        require(
            listed,
            "dfsdjfk"
        );

        uint256 matterTokenAmount = _sendingAmount * (matterTokenPrice / tokenPrice) / collateralFactorMantissa;
        
        suppliersCount = governance.borrowToTrustedAddress(matterTokenId, BORROWING_COEFF * matterTokenAmount);
        require(
            suppliersCount > 0,
            "dsf"
        );

        // exchange with compound
        borrowFromCompound(matterTokenId, matterTokenAmount, _tokenId, _sendingAmount);

        return (matterTokenId, BORROWING_COEFF * matterTokenAmount, suppliersCount);
    }

    function borrowFromCompound(uint16 _tokenSupplyId,
                                uint256 _amountTokenSupply,
                                uint16 _tokenBorrowId,
                                uint256 _amountTokenBorrow)
    internal
    {
        address supplyTokenAddress = governance.validateTokenId(_tokenSupplyId);
        address cSupplyTokenAddress = governance.getCTokenAddress(_tokenSupplyId);

        address cBorrowTokenAddress = governance.getCTokenAddress(_tokenSupplyId);

        address[] memory ctokens = new address[](2);
        ctokens[0] = cSupplyTokenAddress;
        ctokens[1] = cBorrowTokenAddress;
        uint[] memory errors = comptroller.enterMarkets(ctokens);
        require(
            errors[0] == 0 && errors[1] == 0,
            "sebd11"
        ); // sebd11 - enter market failed

        if (_tokenSupplyId == 0) {
            CEther cToken = CEther(cSupplyTokenAddress);
            require(
                cToken.mint.value(_amountTokenSupply)() == 0,
                "sebd12"
            ); // sebd12 - token mint failed
        } else {
            uint256 allowence = IERC20(supplyTokenAddress).allowence(address(this), address(cSupplyTokenAddress));
            if (allowence < _amountTokenSupply) {
                require(
                    IERC20(supplyTokenAddress).approve(address(cSupplyTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "sebd13"
                ); // sebd13 - token approve failed
            }
            CErc20 cToken = CErc20(cSupplyTokenAddress);
            require(
                cToken.mint(_amountTokenSupply) == 0,
                "sebd14"
            ); // sebd14 - token mint failed
        }

        if (_tokenBorrowId == 0) {
            CEther cToken = CEther(cBorrowTokenAddress);
            require(
                cToken.borrow.value(_amountTokenBorrow)() == 0,
                "ssbd14"
            );  // ssbd14 - token borrow failed
        } else {
            CErc20 cToken = CErc20(cBorrowTokenAddress);
            require(
                cToken.borrow(_amountTokenBorrow) == 0,
                "ssbd15"
            );  // ssbd15 - token borrow failed
        }
    }

    /// @notice Repays specified amount to compound
    /// @param _tokenBorrowId Token borrow id
    /// @param _borrowAmount Amount of tokens to repay
    /// @param _tokenSupplyId Token supply id
    /// @param _supplyAmount Amount of supplied tokens
    function repayToCompound(uint16 _tokenBorrowId,
                             uint256 _borrowAmount,
                             uint16 _tokenSupplyId,
                             uint256 _supplyAmount)
    internal
    {
        address tokenAddress = governance.validateTokenId(_tokenBorrowId);
        address cTokenAddress = governance.getCTokenAddress(_tokenBorrowId);

        if (_tokenBorrowId == 0) {
            CEther cToken = CEther(cTokenAddress);
            require(
                cToken.repayBorrow.value(_borrowAmount)() == 0,
                "serd11"
            );  // serd11 - token repay failed
        } else {
            uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(cTokenAddress));
            if (allowence < _borrowAmount) {
                require(
                    IERC20(tokenAddress).approve(address(cTokenAddress), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                    "serd12"
                );  // serd12 - token approve failed
            }
            CErc20 cToken = CErc20(cTokenAddress);
            require(
                cToken.repayBorrow(_borrowAmount) == 0,
                "serd13"
            );  // serd13 - token repay failed
        }

        address cSupplyTokenAddress = governance.getCTokenAddress(_tokenSupplyId);

        if (_tokenSupplyId == 0) {
            CEther cToken = CEther(cSupplyTokenAddress);
            require(
                cToken.redeemUnderlying(_supplyAmount) == 0,
                "serd14"
            );  // serd14 - token redeem failed
        } else {
            CErc20 cToken = CErc20(cSupplyTokenAddress);
            require(
                cToken.redeemUnderlying(_supplyAmount) == 0,
                "serd15"
            );  // serd15 - token redeem failed
        }
    }

    /// @notice Sends specified amount of token to recipient
    /// @param _recipient Recipient address
    /// @param _tokenId Token id
    /// @param _amount Token amount
    function sendTokensToRecipient(address _recipient,
                                   uint16 _tokenId,
                                   uint256 _amount)
    internal
    {
        address tokenAddress = governance.validateTokenId(_tokenId);
        if (tokenAddress == address(0)) {
            _recipient.transfer(_amount);
        } else {
            require(
                IERC20(tokenAddress).transfer(_recipient, _amount),
                "sstt11"
            ); // sstt11 - token transfer out failed
        }
    }

    function fulfillBlock(uint32 _blockNumber)
    external
    {
        require(
            rollup.totalBlocksVerified >= totalBlocksVerified,
            "sfadfdaf"
        );
        uint64 onchainOpsStartIdInBlock = rollup.blocks[_blockNumber].startId;
        for (uint64 i = 0; i < exitOrdersCount[_blockNumber]; i++) {
            ExitOrder order = exitOrders[_blockNumber][i];
            uint256 realOpHash = uint256(keccak256(rollup.onchainOps[onchainOpsStartIdInBlock + order.onchainOpNumber].pubData));
            uint256 expectedOpHash = order.opHash;
            if (realOpHash == expectedOpHash) {
                fulfillSuccededOrder(order);
            } else {
                punishForFailedOrder(order);
            }
        }
    }

    /// @notice Fulfills all succeeded orders
    /// @dev Repays to compound and reduces total borrowed, sends fee to validators balance on compound
    function fulfillSuccededOrder(ExitOrder _order)
    internal
    {
        // Withdraw from rollup
        rollup.withdrawToTrustedAddress(_order.tokenId,
                                        _order.sendingAmount + _order.creationCost + _order.validatorsFee);
        
        // Repay to compound and governance if needed
        if (_order.supplyAmount > 0) {
            if (_order.tokenId != _order.supplyTokenId) {
                repayToCompound(_order.tokenId,
                                _order.sendingAmount,
                                _order.supplyTokenId,
                                _order.supplyAmount / BORROWING_COEFF);
            }
            if (_order.supplyTokenId == 0) {
                governance.repayInEther.value(_order.supplyAmount)
                                             (_order.suppliersCount,
                                              0);
            } else {
                address tokenAddress = governance.validateTokenId(supplyTokenId);
                uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
                if (allowence < _order.supplyAmount) {
                    require(
                        IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                        "sebd13"
                    ); // sebd13 - token approve failed
                }
                governance.repayInErc20(_order.supplyTokenId,
                                        _order.supplyAmount,
                                        _order.suppliersCount,
                                        0);
            }
        }

        // Consummate fees
        if (_order.tokenId == 0) {
            governance.repayInEther.value(_order.validatorsFee);
            governance.supplyEther.value(_order.creationCost)(order.validatorSender);
        } else {
            governance.repayInErc20(_order.tokenId,
                                    _order.validatorsFee);
            governance.supplyErc20(_order.tokenId,
                                   _order.creationCost,
                                   order.validatorSender);
        }
    }

    /// @notice Punishes for failed orders
    /// @dev Reduces validators supplies for failed orders, reduces total borrow
    /// @param _succeededHashes Succeeded orders hashes
    function punishForFailedOrder(ExitOrder _order)
    internal
    {
        // Defrost funds on rollup
        rollup.defrostFunds(
            _order.tokenId,
            _order.sendingAmount + _order.creationCost + _order.validatorsFee,
            recipient
        );

        // Repay to governance
        if (_order.supplyAmount > 0) {
            if (_order.supplyTokenId == 0) {
                governance.repayInEther.value(_order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF)
                                             (_order.suppliersCount,
                                              _order.signersBitmask);
            } else {
                address tokenAddress = governance.validateTokenId(supplyTokenId);
                uint256 allowence = IERC20(tokenAddress).allowence(address(this), address(governance));
                if (allowence < _order.supplyAmount) {
                    require(
                        IERC20(tokenAddress).approve(address(governance), 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
                        "sebd13"
                    ); // sebd13 - token approve failed
                }
                governance.repayInErc20(_order.supplyTokenId,
                                        _order.supplyAmount * (BORROWING_COEFF - 1) / BORROWING_COEFF,
                                        _order.suppliersCounts,
                                        _order.signersBitmask);
            }
        }
    }
}
