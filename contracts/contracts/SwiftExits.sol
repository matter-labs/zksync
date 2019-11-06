pragma solidity ^0.5.8;

import "./Governance.sol";
import "./Franklin.sol";
import "./BlsVerifier.sol";
import "./SafeMath.sol";
import "./BlsOperations.sol";

/// @title Lending Token Contract
/// @notice Inner logic for LendingEther and LendingErc20 token contracts
/// @author Matter Labs
contract LendingToken is BlsVerifier {
    
    using SafeMath for uint256;

    /// @notice Possible price change coeff on Compound
    uint256 constant possiblePriceRisingCoeff = 110/100;

    /// @notice Owner of the contract (Matter Labs)
    address internal owner;

    /// @notice Matter token contract address
    address internal matterToken;

    /// @notice blsVerifier contract
    BlsVerifier internal blsVerifier;

    /// @notice Governance contract
    Governance internal governance;

    /// @notice Rollup contract
    Franklin internal rollup;

    /// @notice last verified Fraklin block
    uint256 internal lastVerifiedBlock;

    /// @notice total funds supply on contract
    uint256 internal totalSupply;

    /// @notice total funds borrowed on contract
    uint256 internal totalBorrowed;

    /// @notice Validators addresses list
    mapping(uint32 => address) internal validators;
    /// @notice Each validators' info
    mapping(address => ValidatorInfo) internal validatorsInfo;
    /// @notice validators count
    uint32 internal validatorsCount;

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
    mapping(uint256 => mapping(uint32 => BorrowOrder)) internal BorrowOrders;
    /// @notice Borrow orders in Swift Exits count
    mapping(uint256 => uint32) internal borrowOrdersCount;

    /// @notice Container for information about validator
    /// @member exists Flag for validator existance in current lending process
    /// @member pubkey Validators' pubkey
    /// @member supply Validators' supplied funds
    struct ValidatorInfo {
        bool exists;
        BlsOperations.G2Point pubkey;
        uint256 supply;
    }

    /// @notice Container for information about borrow order
    /// @member borrowed This orders' borrow
    /// @member validator Borrow orders' recipient (validator)
    struct BorrowOrder {
        uint256 borrowed;
        uint256 fee;
        address validator;
    }

    /// @notice Container for information about Swift Exit Order
    /// @member status Order status
    /// @member withdrawOpOffset Withdraw operation offset in block
    /// @member tokenId Order token id
    /// @member tokenAmount Order token amount
    /// @member recipient Recipient address
    struct ExitOrder {
        ExitOrderState status;
        uint64 withdrawOpOffset;
        uint16 tokenId;
        uint256 tokenAmount;
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
    function setupRelatedContracts(
        address _matterTokenAddress,
        address _governanceAddress,
        address _rollupAddress,
        address _blsVerifierAddress
    ) external {
        requireOwner();
        governance = Governance(_governanceAddress);
        rollup = Franklin(_rollupAddress);
        lastVerifiedBlock = rollup.totalBlocksVerified;
        blsVerifier = BlsVerifier(_blsVerifierAddress);
        matterToken = _matterTokenId;
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
    function transferOut(address _to, uint16 _tokenAddress, uint256 _amount) internal {
        if (_tokenAddress == address(0)) {
            _to.transfer(_amount);
        } else {
            require(
                IERC20(_tokenAddress).transfer(_to, _amount),
                "sstt11"
            ); // sstt11 - token transfer out failed
        }
    }
    
    /// @notice Adds new validator
    /// @dev Only owner can add new validator
    /// @param _address Validator address
    /// @param _pbkxx Validator pubkey xx
    /// @param _pbkxy Validator pubkey xy
    /// @param _pbkyx Validator pubkey yx
    /// @param _pbkyy Validator pubkey yy
    function addValidator(
        address _address,
        uint256 _pbkxx,
        uint256 _pbkxy,
        uint256 _pbkyx,
        uint256 _pbkyy
    ) external {
        requireOwner();
        require(
            !validatorsInfo[_address].exists,
            "ssar11"
        ); // ssar11 - operator exists
        validatorsInfo[_address].exists = true;
        validatorsInfo[_address].pubkey = BlsOperations.G2Point({
                x: [
                    _pbkxx,
                    _pbkxy
                ],
                y: [
                    _pbkyx,
                    _pbkyy
                ]
        });
        validatorsCount++;
    }

    /// @notice Gets allowed withdraw amount for validator
    /// @dev Requires validators' existance
    /// @param _address Validator address
    function getAllowedWithdrawAmount(address _address) public returns (uint256) {
        if (totalSupply-totalBorrowed >= validatorsInfo[_address].supply) {
            return validatorsInfo[_address].supply;
        } else {
            return totalSupply-totalBorrowed;
        }
    }

    /// @notice Withdraws specified amount from validators supply
    /// @dev Requires validators' existance and allowed amount is >= specified amount, which should not be equal to 0
    /// @param _amount Specified amount
    function withdrawForValidator(uint256 _amount) external {
        require(
            getAllowedWithdrawAmount(msg.sender) >= _amount,
            "sswr11"
        ); // sswr11 - wrong amount
        immediateWithdraw(amount);
    }

    /// @notice Withdraws possible amount from validators supply
    /// @dev Requires validators' existance and allowed amount > 0
    function withdrawPossibleForValidator() external {
        uint256 amount = getAllowedWithdrawAmount(msg.sender);
        immediateWithdraw(amount);
    }

    /// @notice The specified amount of funds will be withdrawn from validators supply
    /// @param _amount Specified amount
    function immediateWithdraw(uint256 _amount) internal {
        require(
            _amount > 0,
            "ssir11"
        ); // ssir11 - wrong amount
        transferOut(msg.sender, matterToken, _amount);
        totalSupply -= _amount;
        validatorsInfo[msg.sender].supply -= _amount;
    }

    /// @notice Removes validator for current processing list
    /// @dev Requires owner as sender and validators' existance
    /// @param _address Validator address
    function removeValidator(
        address _address
    ) external {
        requireOwner();
        require(
            validatorsInfo[_address].exists,
            "ssrr11"
        ); // ssrr11 - operator does not exists

        validatorsInfo[_address].exists = false;

        bool found = false;
        for (uint32 i = 0; i < validatorsCount-2; i++){
            if (found || validators[i] == _address) {
                found = true;
                validators[i] = validators[i+1];
            }
        }
        delete validators[validatorsCount-1];
        validatorsCount--;
    }

    /// @notice Supplies specified amount of tokens from validator
    /// @dev Calls transferIn function of specified token and fulfillDefferedWithdrawOrders to fulfill deffered withdraw orders
    /// @param _address Validator account address
    /// @param _amount Token amount
    function supplyValidator(address _address, uint256 _amount) public {
        require(
            validatorsInfo[_address].exists,
            "sssr11"
        ); // sssr11 - operator does not exists
        transferInERC20(_address, matterToken, _amount);
        totalSupply = _address.add(_amount);
        validatorsInfo[_address] += _amount;
    }

    /// @notice Returns validators pubkeys for specified validators addresses
    /// @param _validators Validators addresses
    function getValidatorsPubkeys(address[] memory _validators) internal returns (BlsOperations.G2Point[] memory pubkeys) {
        for (uint32 i = 0; i < _validators.length; i++) {
            pubkeys[i] = validatorsInfo[_validators[i]].pubkey;
        }
        return pubkeys;
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
            _validators.length > 0,
            "ssat11"
        ); // "ssat11" - validators count must be > 0
        if (_validators.length > 1) {
            require(
                blsVerifier.verifyBlsSignature(
                    G1Point(_aggrSignatureX, _aggrSignatureY),
                    getValidatorsPubkeys(_validators)
                ),
                "ssat12"
            ); // "ssat12" - wrong signature
        }
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
            (uint256 amountToExchange, uint256 validatorsFee, uint256 ownerFee) = getAmountToExchange(_tokenId, _tokenAmount);

            // Create Exit orer
            ExitOrder order = ExitOrder(
                ExitOrderState.None,
                _withdrawOpOffset,
                _tokenId,
                _tokenAmount,
                _recipient
            );
            exitOrdersHashes[_blockNumber][exitOrdersCount[_blockNumber]] = _withdrawOpHash;
            exitOrders[_withdrawOpHash] = order;
            exitOrdersCount[_blockNumber]++;

            if (amountToExchange <= (totalSupply - totalBorrowed)) {
                // If amount to borrow <= borrowable amount - immediate swift exit
                immediateSwiftExit(_blockNumber, _withdrawOpHash, amountToExchange, validatorsFee, ownerFee);
            } else {
                // If amount to borrow > borrowable amount - deffered swift exit order
                exitOrders[_withdrawOpHash].status = ExitOrderState.Deffered;
                emit UpdatedExitOrder(
                    _blockNumber,
                    _withdrawOpHash,
                    (amountToExchange * possiblePriceRisingCoeff) - totalSupply + totalBorrowed
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
        (uint256 amountToExchange, uint256 validatorsFee, uint256 ownerFee) = getAmountToExchangeAndFees(order.tokenId, order.tokenAmount);

        if (amountToExchange <= (totalSupply - totalBorrowed)) {
            // If amount to borrow <= borrowable amount - immediate swift exit
            immediateSwiftExit(_blockNumber, _withdrawOpHash, amountToExchange, validatorsFee, ownerFee);
        } else {
            // If amount to borrow > borrowable amount - emit update deffered swift exit order event
            updatedAmount = (amountToExchange * possiblePriceRisingCoeff) - totalSupply + totalBorrowed;
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
    /// @param _amountToExchange Amount to borrow from validators and exchange with compound
    /// @param _validatorsFee Amount of validators fee
    /// @param _ownerFee Amount of owner fee
    function immediateSwiftExit(
        uint32 _blockNumber,
        uint256 _withdrawOpHash,
        uint256 _amountToExchange,
        uint256 _validatorsFee,
        uint256 _ownerFee
    ) internal {
        ExitOrder order = exitOrders[_withdrawOpHash];

        uint256 recievedTokenAmount = borrowFromCompound(order.tokenId, _amountToExchange);
        
        exchangeWithRecipient(order.recipient, order.tokenId, recievedTokenAmount);

        createBorrowOrders(_withdrawOpHash, _amountToExchange, _validatorsFee, _ownerFee);

        rollup.orderSwiftExit(_blockNumber, order.withdrawOpOffset, _withdrawOpHash, order.recipient);

        exitOrders[_withdrawOpHash].tokenAmount = recievedTokenAmount;
        exitOrders[_withdrawOpHash].status = ExitOrderState.Fulfilled;
        totalBorrowed += _amountToExchange;
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
    /// @param _tokenId Token id
    /// @param _amount Amount of validators supply to exchange for token
    function borrowFromCompound(
        uint16 _tokenId,
        uint256 _amount
    ) internal returns (uint256 tokenAmount) {
        address tokenAddress = governance.tokenAddresses(_tokenId);
        // register token on compount if needed
        // exchange possible value
    }

    /// @notice Repays specified amount to compound
    /// @param _tokenId Token id
    /// @param _amount Amount of validators supply to exchange for token
    function repayToCompound(
        uint16 _tokenId,
        uint256 _amount
    ) internal {
        address tokenAddress = governance.tokenAddresses(_tokenId);
        // register token on compount if needed
        // exchange possible value
    }

    /// @notice Returns amound of validators supply to exchange for token, calculates fees
    /// @param _tokenId Token id
    /// @param _tokenAmount Token amount
    function getAmountToExchangeAndFees(
        uint16 _tokenId,
        uint256 _tokenAmount
    ) internal returns (uint256 amountToExchange, uint256 validatorsFee, uint256 ownerFee) {
        address tokenAddress = governance.tokenAddresses(_tokenId);
        address matterAddress = governance.tokenAddresses(matterToken);

        uint256 collateralFactor = compound.takeCollateralFactor(matterAddress);

        uint256 tokenPrice = compound.takePrice(tokenAddress);
        
        uint256 fullAmount = _tokenAmount * tokenPrice / collateralFactor;

        amountToExchange = 0.9 * fullAmount;
        validatorsFee = 0.095 * _tokenAmount;
        ownerFee = 0.095 * _tokenAmount;
    }

    /// @notice Creates borrow orders for specified exit request
    /// @param _withdrawOpHash Withdraw operation hash
    /// @param _amountToBorrow Amount of validators supply to borrow
    /// @param _validatorsFee Amount of validators fee
    /// @param _ownerFee Amount of owner fee
    function createBorrowOrders(
        uint256 _withdrawOpHash,
        uint256 _amountToBorrow,
        uint256 _validatorsFee,
        uint256 _ownerFee
    ) internal {
        for (uint32 i = 0; i <= validatorCount; i++) {
            uint32 currentBorrowOrdersCount = borrowOrdersCount[_withdrawOpHash];
            BorrowOrders[_withdrawOpHash][currentBorrowOrdersCount] = BorrowOrder({
                borrowed: _amountToBorrow * (validatorsInfo[validators[i]].supply / totalSupply),
                fee: _validatorsFee * (validatorsInfo[validators[i]].supply / totalSupply),
                validator: validators[i]
            });
            borrowOrdersCount[_withdrawOpHash]++;
        }
        uint32 currentBorrowOrdersCount = borrowOrdersCount[_withdrawOpHash];
        BorrowOrders[_withdrawOpHash][currentBorrowOrdersCount] = BorrowOrder({
            borrowed: 0,
            fee: _ownerFee,
            validator: validators[owner]
        });
        borrowOrdersCount[_withdrawOpHash]++;
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
        address[] calldata _tokenAddresses,
        uint256[] calldata _tokenAmounts
    ) external payable {
        requireRollup();
        require(
            _tokenAddresses.length == _tokenAmounts.length,
            "ssnk11"
        ); // "ssnk11" - token addresses array length must be equal token amounts array length
        for (uint i = 0; i < token.length; i++) {
            transferInERC20(msg.sender, _tokenAddresses[i], _tokenAmounts[i]);
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
            repayToCompound(exitOrder.tokenId, exitOrder.tokenAmount);
            for (uint32 k = 0; k < borrowOrdersCount[_succeededHashes[i]]; k++) {
                BorrowOrder order = BorrowOrders[_succeededHashes[i]][k];
                totalBorrowed -= order.borrowed;
                sendFeeToRollup(order.validator, exitOrder.tokenId, order.fee);
            }
        }
    }

    /// @notice Sends fees to rollup
    /// @param _validator Validator address
    /// @param _tokenId Token id
    /// @param _amount Token amount
    function sendFeeToRollup(address _validator, uint16 _tokenId, uint256 _fee) internal {
        if (_fee > 0) {
            address tokenAddr = governance.tokenAddresses(tokenId);
            if (tokenId > 0) {
                require(
                    IERC20(tokenAddr).approve(address(rollup), _fee),
                    "fw011"
                ); // fw011 - token approve failed
            }
            rollup.depositOnchain(_validator, _tokenId, _fee);
        }
    }

    /// @notice Fulfills all deffered orders
    /// @dev Instantly sends from rollup to recipient for all deffered orders from specified block
    /// @param _blockNumber Block number
    function fulfillDefferedExitOrders(uint32 _blockNumber) internal {
        for (uint32 i = 0; i < exitOrdersCount[_blockNumber]; i++) {
            ExitOrder order = exitOrders[exitOrdersHashes[i]];
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
            for (uint32 k = 0; k < borrowOrdersCount[_failedHashes[i]]; k++) {
                BorrowOrder order = BorrowOrders[_succeededHashes[i]][k];
                if (order.borrowed > 0) {
                    validatorsInfo[order.validator].supply -= order.borrowed;
                    totalSupply -= order.borrowed;
                    totalBorrowed -= order.borrowed;
                }
            }
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
