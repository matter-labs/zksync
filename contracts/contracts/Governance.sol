pragma solidity ^0.5.8;

import "./IERC20.sol";
import "./BlsOperations.sol";
import "./SwiftExits.sol";
import "./Franklin.sol";
import "./SignaturesVerifier.sol";

/// @title Governance Contract
/// @author Matter Labs
contract Governance {

    /// @notice Freeze time for token, when validator withdraw it. Then the validator must wait this time to withdraw it again
    uint256 constant FREEZE_TIME = 10000;
    
    /// @notice Validator-creator fee coefficient
    uint256 constant VALIDATOR_CREATOR_FEE_COEFF = 10;

    /// @notice Rollup contract, contains user funds
    Franklin rollup;

    /// @notice SwiftExits contract, processes swift exits request
    SwiftExits swiftExits;

    /// @notice Matter token id
    uint16 public matterTokenId;

    /// @notice Matter token address
    address public matterTokenAddress;

    /// @notice Compound CMatter token address
    address public cMatterTokenAddress;

    /// @notice Address which will excercise governance over the network i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    /// @notice Total number of ERC20 tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    /// @notice validators count
    uint16 public totalValidators;

    /// @notice validators total supplied Matter tokens value
    uint256 public totalSupply;
    
    /// @notice validators total lended Matter tokens value
    uint256 public totalLended;

    /// @notice List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    /// @notice List of registered cTokens (corresponding to underlying token on Compound) by corresponding token id
    mapping(uint16 => address) public cTokenAddresses;

    /// @notice List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    /// @notice Validators addresses list
    mapping(uint16 => address) public validators;

    /// @notice Each validators' info
    mapping(address => ValidatorInfo) public validatorsInfo;
    
    /// @notice Each validators' frozen to withdraw tokens and block number until which the freezing acts
    mapping(address => mapping(uint16 => uint256)) public validatorsFrozenTokens;

    /// @notice Accumulated fees on contract
    mapping(uint16 => uint256) public accumulatedFees;
    
    /// @notice Container for information about validator
    /// @member supply Supplied Matter token id
    /// @member isActive Flag for validator existance in current lending process
    /// @member id Validator id. Needed to identify single validator in bitmask
    /// @member pubkey Validators' pubkey
    /// @member frozenUntilBlock Indicates that all validators tokens are unavailable to withdraw until thatblock number
    struct ValidatorInfo {
        uint256 supply;
        bool isActive;
        uint16 id;
        BlsOperations.G2Point pubkey;
        uint256 frozenUntilBlock;
    }

    /// @notice Token added to Franklin net
    /// @member token Token address
    /// @member tokenId Token id
    event TokenAdded(
        address token,
        uint16 tokenId
    );

    /// @notice cToken (corresponding to underlying token on Compound) added to Franklin net
    /// @member cToken cToken address
    /// @member token Underlying token id
    event cTokenAdded(
        address cToken,
        uint16 tokenId
    );

    /// @notice Construct Governance contract
    /// @param _networkGovernor The address of governor
    constructor(address _networkGovernor) external {
        networkGovernor = _networkGovernor;
    }

    /// @notice Add addresses of related contracts
    /// @dev Requires governor. MUST be called before all other operations
    /// @param _matterTokenAddress The address of Matter token
    /// @param _cMatterTokenAddress The address of Compound CMatter token
    /// @param _cEtherTokenAddress The address of Compound CEther token
    /// @param _rollupAddress The address of Rollup contract
    /// @param _swiftExitsAddress The address of SwiftExits contract
    function setupRelatedContracts(
        address _matterTokenAddress,
        address _cMatterTokenAddress,
        address _cEtherTokenAddress,
        address _rollupAddress,
        address _swiftExitsAddress
    ) external {
        require(
            _matterTokenAddress == address(0) &&
            _cMatterTokenAddress == address(0) &&
            _cEtherTokenAddress == address(0) &&
            _rollupAddress == address(0) &&
            _swiftExitsAddress == address(0),
            "gess11"
        ); // gess11 - contracts must be setted only once

        // Can be called only by governor
        requireGovernor();

        // Set contracts
        swiftExits = SwiftExits(_swiftExitsAddress);
        rollup = Franklin(_rollupAddress);

        // Add matter token
        addToken(_matterTokenAddress);
        // Add cMatter token
        addCToken(_cMatterTokenAddress, totalTokens);

        // Set Matter token address
        matterTokenAddress = _matterTokenAddress;
        // Set cMatter token address
        cMatterTokenAddress = _cMatterTokenAddress;
        // Save Matter token id
        matterTokenId = totalTokens;

        // Add cEther
        addCToken(_cEtherTokenAddress, 0);
    }

    /// @notice Fallback function
    /// @dev Reverts all payments in Ether
    function() external payable {
        revert("Cant accept ether in fallback function");
    }

    /// @notice Change current governor
    /// @param _newGovernor Address of the new governor
    function changeGovernor(address _newGovernor) external {
        // Can be called only by governor
        requireGovernor();

        networkGovernor = _newGovernor;
    }

    /// @notice Check if the sender is governor
    function requireGovernor() public view {
        require(
            msg.sender == networkGovernor,
             "gerr11"
        ); // gerr11 - only by governor
    }

    /// @notice Add token to the list of networks tokens
    /// @param _token Token address
    function addToken(address _token) public {
        // Can be called only by governor
        requireGovernor();
        // Token must be added once
        require(
            tokenIds[_token] == 0,
             "gean11"
        ); // gean11 - token exists

        // Adding one to token id because tokenId = 0 is reserved for ETH
        tokenAddresses[totalTokens + 1] = _token;
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

    /// @notice Add cToken for token to the list of cTokens. cToken is Compound representation for underlying token
    /// @param _cToken cToken address
    /// @param _token Underlying token address
    function addCToken(address _cToken, uint16 _tokenId) external {
        // Can be called only by governor
        requireGovernor();
        // Token must not be Ether
        require(
            validateTokenId(_tokenId) != address(0),
             "gean21"
        ); // gean21 - token with specified id doenst exists
        cTokenAddresses[_tokenId] = _cToken;
        emit cTokenAdded(_cToken, _tokenId);
    }

    /// @notice Validate token address and returns its id
    /// @param _tokenAddr Token address
    function validateTokenAddress(address _tokenAddr) public view returns (uint16) {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(
            tokenAddresses[tokenId] == _tokenAddr,
             "gevs11"
        ); // gevs11 - unknown ERC20 token address
        return tokenId;
    }

    /// @notice Validate token id and returns its address
    /// @param _tokenId Token id
    function validateTokenId(uint16 _tokenId) public view returns (address) {
        uint16 tokenAddr = tokenAddresses[_tokenId];
        require(
            tokenIds[tokenAddr] == _tokenId,
             "gevs11"
        ); // gevs11 - unknown ERC20 token id
        return tokenAddr;
    }

    /// @notice Add new validator with pubkey
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
    )
        external
    {
        // Can be called only by governor
        requireGovernor();
        // Requires validator not to be active
        require(
            !validatorsInfo[_address].isActive,
            "gear11"
        ); // gear11 - operator exists
        validatorsInfo[_address].isActive = true;
        validatorsInfo[_address].id = totalValidators;
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
        validators[totalValidators] = _address;
        totalValidators++;
    }

    /// @notice Change validator status (active or not active)
    /// @param _address Validator address
    /// @param _active Active flag
    function setValidatorStatus(address _address, bool _active) external {
        // Can be called only by governor
        requireGovernor();
        // Require validator to exist (non-zero pubkey)
        require(
            validatorsInfo[_address].pubkey.x[0] != 0 &&
            validatorsInfo[_address].pubkey.x[1] != 0 &&
            validatorsInfo[_address].pubkey.y[0] != 0 &&
            validatorsInfo[_address].pubkey.y[1] != 0,
            "gess11"
        ); // gess11 - operator pubkey must exist
        validatorsInfo[_address].isActive = _active;
    }

    /// @notice Sends the swift exit request, signed by user and validators, to SwiftExits contract,
    /// @notice borrows tokens for it from validators, freezes tokens on rollup contract
    /// @param _swiftExit Signed swift exit data: block number, onchain op number, acc number, token id, token amount, fee amount, recipient, author
    /// @param _userSignature User signature
    /// @param _signersSignature Aggregated validators signature
    function createSwiftExitRequest(
        bytes memory _swiftExit,
        bytes memory _userSignature,
        bytes memory _signersSignature,
        uint16 _signersBitmask
    ) external {
        // Signers bitmask must not be nill
        require(
            _signersBitmask > 0,
            "gect11"
        ); // "gect11" - there must be signers

        // Swift Exit data:
        // blockNumber Rollup block number
        // onchainOpNumber Withdraw operation number in block
        // accNumber Account - creator of withdraw operation
        // tokenId Token id
        // tokenAmount Token amount
        // feeAmount Fee amount in specified tokens
        // recipient Withdraw operation recipient
        // owner Withdraw operation owner
        // swiftExitFee Swift exit fee for validators, signed by user
        // supplyAmount Validators supplied amount to fulfill this requests
        (
            uint32 blockNumber,
            uint64 onchainOpNumber,
            uint24 accNumber,
            uint16 tokenId,
            uint256 tokenAmount,
            uint16 feeAmount,
            address recipient,
            address owner,
            uint256 swiftExitFee,
            uint256 supplyAmount
        ) = parseSwiftExit(_swiftExit);

        // Checks that amounts are enouth

        require(
            tokenAmount > 0,
            "gect12"
        ); // "gect12" - token amount must be > 0

        require(
            supplyAmount > 0,
            "gect13"
        ); // "gect13" - supply amount must be > 0

        require(
            swiftExitFee > 0,
            "gect14"
        ); // "gect14" - fees must be > 0

        require(
            tokenAmount > swiftExitFee,
            "ssat15"
        ); // "ssat15" - amount must be > fees

        // Check that there are enouth free tokens on contract
        require(
            (2 * totalSupply / 3) - totalLended >= supplyAmount,
            "gect16"
        ); // "gect16" - not enouth amount

        // Check that sender exists in bitmask and verify validators signature
        require(
            verifySenderPresenceAndValidatorsSignature(
                msg.sender, // Sender MUST be active validator
                _signersSignature,
                _signersBitmask,
                uint256(keccak256(_swiftExit))
            ),
            "gect17"
        ); // "gect17" - wrong signature or validator-sender is not in signers bitmask
        
        // Send tokens to swiftExits
        require(
            IERC20(matterTokenAddress).transfer(address(swiftExits), supplyAmount),
            "gect18"
        ); // gect18 - token transfer out failed

        // Increase total lended balance
        totalLended += supplyAmount;

        // Freeze tokenAmount on Rollup contract for recipient
        rollup.freezeFunds(
            blockNumber,
            onchainOpNumber,
            accNumber,
            tokenId,
            tokenAmount,
            feeAmount,
            recipient,
            owner,
            swiftExitFee,
            _userSignature
        );

        // Save the swift exit on SwiftExits contract
        swiftExits.addSwiftExit(
            blockNumber,
            onchainOpNumber,
            accNumber,
            tokenId,
            tokenAmount,
            feeAmount,
            recipient,
            owner,
            swiftExitFee,
            supplyAmount
        );
    }

    /// @notice Checks if validator is active
    /// @param _address Validator address
    function requireActiveValidator(address _address) public {
        require(
            validatorsInfo[_address].isActive,
            "geir11"
        ); // geir11 - validator is not active
    }
    
    /// @notice Parses swift exit bytes data into its field
    /// @param _swiftExit Swift exit bytes data
    function parseSwiftExit(bytes memory _swiftExit) internal returns (
        uint32 blockNumber,
        uint64 onchainOpNumber,
        uint24 accNumber,
        uint16 tokenId,
        uint128 tokenAmount,
        uint16 feeAmount,
        address recipient,
        address owner,
        uint256 swiftExitFee,
        uint256 supplyAmount
    ) {
        require(
            _swiftExit.length == 139,
            "gept11"
        ); // gept11 - wrong swift exit length

        uint8 blockNumberBytesLen = 4;
        uint8 onchainOpNumberBytesLen = 8;
        uint8 accNumberBytesLen = 3;
        uint8 tokenIdBytesLen = 2;
        uint8 tokenAmountBytesLen = 16;
        uint8 feeAmountBytesLen = 2;
        uint8 recipientBytesLen = 20;
        uint8 ownerBytesLen = 20;
        uint8 swiftExitFeeBytesLen = 32;
        uint8 supplyAmountBytesLen = 32;

        bytes memory blockNumberBytes = new bytes(blockNumberBytesLen);
        for (uint8 i = 0; i < blockNumberBytesLen; ++i) {
            blockNumberBytes[i] = _swiftExit[i];
        }
        blockNumber = Bytes.bytesToUInt32(blockNumberBytes);

        bytes memory onchainOpNumberBytes = new bytes(onchainOpNumberBytesLen);
        for (uint8 i = 0; i < onchainOpNumberBytesLen; ++i) {
            onchainOpNumberBytes[i] = _swiftExit[blockNumberBytesLen + i];
        }
        onchainOpNumber = Bytes.bytesToUInt64(onchainOpNumberBytes);

        bytes memory accNumberBytes = new bytes(accNumberBytesLen);
        for (uint8 i = 0; i < accNumberBytesLen; ++i) {
            accNumberBytes[i] = _swiftExit[blockNumberBytesLen + onchainOpNumberBytesLen + i];
        }
        accNumber = Bytes.bytesToUInt24(accNumberBytes);

        bytes memory tokenIdBytes = new bytes(tokenIdBytesLen);
        for (uint8 i = 0; i < tokenIdBytesLen; ++i) {
            tokenIdBytes[i] = _swiftExit[blockNumberBytesLen + onchainOpNumberBytesLen + accNumberBytesLen + i];
        }
        tokenId = Bytes.bytesToUInt16(tokenIdBytes);

        bytes memory tokenAmountBytes = new bytes(tokenAmountBytesLen);
        for (uint8 i = 0; i < tokenAmountBytesLen; ++i) {
            tokenAmountBytes[i] = _swiftExit[blockNumberBytesLen + onchainOpNumberBytesLen + accNumberBytesLen + tokenIdBytesLen + i];
        }
        tokenAmount = Bytes.bytesToUInt128(tokenAmountBytes);

        bytes memory feeAmountBytes = new bytes(feeAmountBytesLen);
        for (uint8 i = 0; i < feeAmountBytesLen; ++i) {
            feeAmountBytes[i] = _swiftExit[
                blockNumberBytesLen +
                onchainOpNumberBytesLen +
                accNumberBytesLen +
                tokenIdBytesLen +
                tokenAmountBytesLen +
                i
            ];
        }
        feeAmount = Bytes.bytesToUInt16(feeAmountBytes);

        bytes memory recipientBytes = new bytes(recipientBytesLen);
        for (uint8 i = 0; i < recipientBytesLen; ++i) {
            recipientBytes[i] = _swiftExit[
                blockNumberBytesLen +
                onchainOpNumberBytesLen +
                accNumberBytesLen +
                tokenIdBytesLen +
                tokenAmountBytesLen +
                feeAmountBytesLen +
                i
            ];
        }
        recipient = Bytes.bytesToAddress(recipientBytes);

        bytes memory ownerBytes = new bytes(ownerBytesLen);
        for (uint8 i = 0; i < ownerBytesLen; ++i) {
            ownerBytes[i] = _swiftExit[
                blockNumberBytesLen +
                onchainOpNumberBytesLen +
                accNumberBytesLen +
                tokenIdBytesLen +
                tokenAmountBytesLen +
                feeAmountBytesLen +
                recipientBytesLen +
                i
            ];
        }
        owner = Bytes.bytesToAddress(ownerBytes);

        bytes memory swiftExitFeeBytes = new bytes(swiftExitFeeBytesLen);
        for (uint8 i = 0; i < swiftExitFeeBytesLen; ++i) {
            swiftExitFeeBytes[i] = _swiftExit[
                blockNumberBytesLen +
                onchainOpNumberBytesLen +
                accNumberBytesLen +
                tokenIdBytesLen +
                tokenAmountBytesLen +
                feeAmountBytesLen +
                recipientBytesLen +
                ownerBytesLen +
                i
            ];
        }
        swiftExitFee = Bytes.bytesToUInt256(swiftExitFeeBytes);

        bytes memory supplyAmountBytes = new bytes(supplyAmountBytesLen);
        for (uint8 i = 0; i < supplyAmountBytesLen; ++i) {
            supplyAmountBytes[i] = _swiftExit[
                blockNumberBytesLen +
                onchainOpNumberBytesLen +
                accNumberBytesLen +
                tokenIdBytesLen +
                tokenAmountBytesLen +
                feeAmountBytesLen +
                recipientBytesLen +
                ownerBytesLen +
                swiftExitFeeBytesLen +
                i
            ];
        }
        supplyAmount = Bytes.bytesToUInt256(supplyAmountBytes);
    }

    /// @notice Returns validators aggregated pubkey and their count for specified validators bitmask
    /// @param _bitmask Validators bitmask
    function getValidatorsAggrPubkey(uint16 _bitmask) internal view returns (
        BlsOperations.G2Point memory aggrPubkey,
        uint16 signersCount
    ) {
        // Go into a loop for totalValidators
        for(uint8 i = 0; i < totalValidators; i++) {
            // Check that validator exists in bitmask
            if( (bitmask >> i) & 1 > 0 ) {
                address addr = validators[i];
                // Check that validator is active
                requireActiveValidator(addr);
                // Get her pubkey add it to aggregated pubkey
                BlsOperations.G2Point memory pubkey = validatorsInfo[addr].pubkey;
                aggrPubkey = BlsOperations.addG2(aggrPubkey, pubkey);
                signersCount++;
            }
        }
    }

    /// @notice Verifies sender presence in bitmask and verifies aggregated bls signature
    /// @param _sender Sender of the request
    /// @param _aggrSignature Aggregated signature
    /// @param _signersBitmask Signers bitmask
    /// @param _messageHash Message hash
    function verifySenderPresenceAndValidatorsSignature(
        address _sender,
        bytes memory _aggrSignature,
        uint256 _signersBitmask,
        uint256 _messageHash
    )
        internal
        view
        returns (bool result)
    {
        // If there is only 1 validator and he is sender - return true (single validator)
        if (totalValidators == 1 && validators[0] == _sender) {
            return true;
        }

        // Check if sender is in bitmask
        uint16 validatorId = validatorsInfo[_sender].id;
        require(
            (_signersBitmask >> validatorId) & 1 > 0,
            "geve11"
        ); // geve11 - sender is not in validators bitmask

        // Bls signature veification
        (BlsOperations.G2Point memory aggrPubkey, uint16 signersCount) = getValidatorsAggrPubkey(_signersBitmask);
        require(
            signersCount >= 2 * totalValidators / 3,
            "geve12"
        ); // geve12 - not enouth validators count

        return SignaturesVerifier.verifyValidatorsSignature(
            aggrPubkey,
            _aggrSignature,
            _messageHash
        );
    }

    /// @notice Supplies specified amount of Matter tokens to validator balance
    /// @param _amount Token amount
    function supplyMatterToken(
        uint256 _amount
    )
        external
    {
        require(
            amount > 0,
            "gesn11"
        ); // gesn11 - amount must be > 0

        // Validator must be active
        requireActiveValidator(msg.sender);
        
        // Transfer Matter token from sender to contract
        require(
            IERC20(matterTokenAddress).transferFrom(msg.sender, address(this), _amount),
            "gesn12"
        ); // gesn12 - token transfer in failed

        // Increase total validators supply
        totalSupply += _amount;

        // Increase validator supply
        validatorsInfo[msg.sender].supply += _amount;

        // Freeze all tokens for validator
        validatorsInfo[msg.sender].frozenUntilBlock = block.number + FREEZE_TIME;
    }

    /// @notice Withdraws specified amount of Matter tokens, supplied by validator
    /// @param _amount Specified amount
    function withdrawSupply(uint256 _amount) external {
        // Amount must be > 0
        require(
            _amount > 0,
            "gewy11"
        ); // gewy11 - amount must be > 0

        // Amount must be withdrawable (total supply - total lended >= amount)
        require(
            _amount <= totalSupply - totalLended,
            "gewy12"
        ); // gewy12 - amount must be <= free matter tokens on contract

        // Validator must have enouth supply
        require(
            _amount <= validatorsInfo[msg.sender].supply,
            "gewy13"
        ); // gewy13 - amount must be <= validator supply

        // Transfer Matter token amount to validator (sender)
        require(
            IERC20(matterTokenAddress).transfer(msg.sender, _amount),
            "gewy14"
        ); // gewy14 - token transfer out failed

        // Reduce total supply
        totalSupply -= _amount;

        // Reduce validator supply
        validatorsInfo[msg.sender].supply -= _amount;
    }

    /// @notice Withdraws specified amount of tokens or ether fees
    /// @param _tokenAddress Token address, 0 if address(0)
    function withdrawFees(address _tokenAddress) external {
        // Validate token address and get its id
        uint16 tokenId = validateTokenAddress(_tokenAddress);

        // Require validator to be able to withdraw token at current ethereum block (token is not frozen)
        require(
            validatorsFrozenTokens[msg.sender][tokenId] <= block.number &&
            validatorsInfo[msg.sender].frozenUntilBlock <= block.number,
            "gews11"
        ); // gews11 - validator cant withdraw this token yet

        uint256 amount = accumulatedFees[tokenId] * validatorsInfo[msg.sender] / totalSupply;
        require(
            amount > 0,
            "gews12"
        ); // gews12 - amount must be > 0

        // Freeze token for validator
        validatorsFrozenTokens[msg.sender][tokenId] = block.number + FREEZE_TIME;

        // Reduce accumulated fees for this token
        accumulatedFees[tokenId] -= amount;

        if (tokenId == 0) {
            // withdraw ether
            msg.sender.transfer(amount);
        } else {
            require(
                IERC20(_tokenAddress).transfer(msg.sender, amount),
                "gews13"
            ); // gews13 - token transfer out failed
        }
    }

    /// @notice Repays specified amount of atter token into contract, charges specified amount of tokens or ether as fee into contract
    /// @param _repayAmount Matter token repayment amount
    /// @param _feesTokenAddress Fees token address, address(0) for Ether
    /// @param _feesAmount Fees amount
    /// @param _validatorCreator Validator, that processed order
    function repayBorrowWithFees(
        uint256 _repayAmount,
        address _feesTokenAddress,
        uint256 _feesAmount,
        address _validatorCreator
    ) external payable {
        // Can be called only from swift exit contract  
        require(
            msg.sender == address(swiftExit),
             "gers11"
        ); // gers11 - not swift exit contract addres

        // Repay borrow
        
        // Repayments amount must be higher than 0
        require(
            _repayAmount > 0,
            "gers12"
        ); // gers12 - repay amount must be > 0

        // Transfer Matter token from SwiftExits contract to this contract
        require(
            IERC20(matterTokenAddress).transferFrom(msg.sender, address(this), _repayAmount),
            "gers13"
        ); // gers13 - matter token transfer in failed

        // Reduce total lended balance
        totalLended -= _repayAmount;

        // Charge fees

        // Validate token address and get its id
        uint16 tokenId = validateTokenAddress(_feesTokenAddress);
        if (tokenId == 0) {
            // Token is Ether

            // Requires fees amount to be provided in msg.value and feesAmount param to be nill
            require(
                _feesAmount == 0 && msg.value > 0,
                "gers14"
            ); // gers14 - amount must be == 0 and msg.value > 0

            // Accumulate validators fees
            accumulatedFees += msg.value * (1 - VALIDATOR_CREATOR_FEE_COEFF / 100);
            
            // Repay fee to validator that created request
            _validatorCreator.transfer(msg.value * VALIDATOR_CREATOR_FEE_COEFF / 100);
        } else {
            // Token is ERC20

            // Requires fees amount to be provided in function params and msg.value to be nill
            require(
                _feesAmount > 0 && msg.value == 0,
                "gers15"
            ); // gers15 - amount must be > 0 and msg.value == 0

            // Transfer token for fees from SwiftExits contract to this contract
            require(
                IERC20(_feesTokenAddress).transferFrom(msg.sender, address(this), _feesAmount),
                "gers16"
            ); // gers16 - token transfer in failed

            // Accumulate validators fees
            accumulatedFees += _feesAmount * (1 - VALIDATOR_CREATOR_FEE_COEFF / 100);

            // Repay fee to validator that created request
            require(
                IERC20(_feesTokenAddress).transfer(_validatorCreator, _feesAmount * VALIDATOR_CREATOR_FEE_COEFF / 100),
                "gers17"
            ); // gers17 - token transfer out failed
        }
    }
}