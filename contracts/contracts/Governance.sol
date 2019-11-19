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

    /// @notice Rollup contract
    Franklin rollup;

    /// @notice SwiftExits contract
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

    /// @notice List of registered cTokens by corresponding token id
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
    struct ValidatorInfo {
        uint256 supply;
        bool isActive;
        uint16 id;
        BlsOperations.G2Point pubkey;
    }

    /// @notice Token added to Franklin net
    /// @member token Token address
    /// @member tokenId Token id
    event TokenAdded(
        address token,
        uint16 tokenId
    );

    /// @notice cToken added to Franklin net
    /// @member cToken cToken address
    /// @member token Token id
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
    /// @param _rollupAddress The address of Rollup contract
    /// @param _swiftExitsAddress The address of SwiftExits contract
    function setupRelatedContracts(
        address _matterTokenAddress,
        address _cMatterTokenAddress,
        address _rollupAddress,
        address _swiftExitsAddress
    ) external {
        requireGovernor();
        swiftExits = SwiftExits(_swiftExitsAddress);
        rollup = Franklin(_rollupAddress);

        addToken(_matterTokenAddress);
        addCToken(_cMatterTokenAddress, totalTokens);
        matterTokenAddress = _matterTokenAddress;
        cMatterTokenAddress = _cMatterTokenAddress;
        matterTokenId = totalTokens;
    }

    /// @notice Fallback function
    /// @dev Reverts all payments in Ether
    function() external payable {
        revert("Cant accept ether in fallback function");
    }

    /// @notice Change current governor
    /// @param _newGovernor Address of the new governor
    function changeGovernor(address _newGovernor) external {
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

    /// @notice Add token to the list of possible tokens
    /// @param _token Token address
    function addToken(address _token) public {
        requireGovernor();
        require(
            tokenIds[_token] == 0,
             "gean11"
        ); // gean11 - token exists
        tokenAddresses[totalTokens + 1] = _token; // Adding one because tokenId = 0 is reserved for ETH
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

    /// @notice Add cToken for token to the list of possible cTokens
    /// @param _cToken cToken address
    /// @param _token Token address
    function addCToken(address _cToken, uint16 _tokenId) external {
        requireGovernor();
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
    /// @dev Only governor can add new validator
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
        requireGovernor();
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

    /// @notice Change validator status
    /// @dev Only governor can add new validator
    /// @param _address Validator address
    /// @param _active Active flag
    function setValidatorStatus(address _address, bool _active) external {
        requireGovernor();
        require(
            validatorsInfo[_address].pubkey.x[0] != 0 &&
            validatorsInfo[_address].pubkey.x[1] != 0 &&
            validatorsInfo[_address].pubkey.y[0] != 0 &&
            validatorsInfo[_address].pubkey.y[1] != 0,
            "gess11"
        ); // gess11 - operator pubkey must exist
        validatorsInfo[_address].isActive = _active;
    }

    /// @notice Sends the swift exit request, signed by user and validators, to SwiftExits contract, borrows tokens for it from validators, freezes tokens on rollup contract
    /// @param _swiftExit Signed swift exit data: block number, onchain op number, acc number, token id, token amount, fee amount, recipient, author
    /// @param _userSignature User signature
    /// @param _signersSignature Aggregated validators signature
    function createSwiftExitRequest(
        bytes memory _swiftExit,
        bytes memory _userSignature,
        bytes memory _signersSignature,
        uint16 _signersBitmask
    ) external {
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

        // Check that there are enouth free tokens on contract
        require(
            (2 * totalSupply / 3) - totalLended >= supplyAmount,
            "gect15"
        ); // "gect15" - not enouth amount

        // Verify sender and validators signature
        require(
            verifySenderAndValidatorsSignature(
                msg.sender, // Sender MUST be active validator
                _signersSignature,
                _signersBitmask,
                uint256(keccak256(_swiftExit))
            ),
            "gect16"
        ); // "gect16" - wrong signature or validator-sender is not in signers bitmask
        
        // Send tokens to swiftExits
        require(
            IERC20(matterTokenAddress).transfer(address(swiftExits), supplyAmount),
            "gect17"
        ); // gect17 - token transfer out failed

        // Sum lended balance
        totalLended += supplyAmount;

        // Freeze funds on rollup contract
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

        // Add the swift exit on SwiftExits contract
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
    
    /// @notice Parses swift exit bytes data to its field
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
        for(uint8 i = 0; i < totalValidators; i++) {
            if( (bitmask >> i) & 1 > 0 ) {
                address addr = validators[i];
                requireActiveValidator(addr);
                BlsOperations.G2Point memory pubkey = validatorsInfo[addr].pubkey;
                aggrPubkey = BlsOperations.addG2(aggrPubkey, pubkey);
                signersCount++;
            }
        }
    }

    /// @notice Verifies sender presence in bitmask and aggregated signature
    /// @param _sender Sender of the request
    /// @param _aggrSignature Aggregated signature
    /// @param _signersBitmask Signers bitmask
    /// @param _messageHash Message hash
    function verifySenderAndValidatorsSignature(
        address _sender,
        bytes memory _aggrSignature,
        uint256 _signersBitmask,
        uint256 _messageHash
    )
        internal
        view
        returns (bool result)
    {
        // If there is only 1 validator and he is sender - return true (single operator model)
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
    /// @param _validator Validator address
    function supplyMatterToken(
        uint256 _amount,
        address _validator
    )
        external
    {
        require(
            amount > 0,
            "gesn11"
        ); // gesn11 - amount must be > 0
        requireActiveValidator(_validator);
        require(
            IERC20(matterTokenAddress).transferFrom(msg.sender, address(this), _amount),
            "gesn12"
        ); // gesn12 - token transfer in failed
        totalSupply += _amount;
        validatorsInfo[_validator].supply += _amount;
    }

    /// @notice Withdraws specified amount of Matter tokens, supplied by validator
    /// @param _amount Specified amount
    function withdrawSupply(uint256 _amount) external {
        require(
            _amount > 0,
            "gewy11"
        ); // gewy11 - amount must be > 0
        require(
            _amount <= totalSupply - totalLended,
            "gewy12"
        ); // gewy12 - amount must be <= free matter tokens on contract
        require(
            _amount <= validatorsInfo[msg.sender].supply,
            "gewy13"
        ); // gewy13 - amount must be <= validator supply
        require(
            IERC20(matterTokenAddress).transfer(msg.sender, _amount),
            "gewy14"
        ); // gewy14 - token transfer out failed
        totalSupply -= _amount;
        validatorsInfo[msg.sender].supply -= _amount;
    }

    /// @notice Withdraws specified amount of tokens or ether fees
    /// @param _tokenAddress Token address, 0 if address(0)
    function withdrawFees(address _tokenAddress) external {
        uint16 tokenId = validateTokenAddress(_tokenAddress);

        require(
            validatorsFrozenTokens[msg.sender][tokenId] <= block.number,
            "gews11"
        ); // gews11 - validator cant withdraw this token yet

        uint256 amount = accumulatedFees[tokenId] * validatorsInfo[msg.sender] / totalSupply;
        require(
            amount > 0,
            "gews12"
        ); // gews12 - amount must be > 0

        // Freeze token for validator
        validatorsFrozenTokens[msg.sender][tokenId] = block.number + FREEZE_TIME;

        // Withdraw fees
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

    /// @notice Repays specified amount of matter token into contract, charges specified amount of tokens or ether as fee into contract
    /// @param _repayAmount Matter token repayment amount
    /// @param _feesTokenAddress Fees token address, address(0) for Ether
    /// @param _feesAmount Fees amount
    function repayBorrowWithFees(
        uint256 _repayAmount,
        address _feesTokenAddress,
        uint256 _feesAmount
    ) external payable {
        require(
            msg.sender == address(swiftExit),
             "gers11"
        ); // gers11 - not swift exit contract addres

        // Repay borrow

        require(
            _repayAmount > 0,
            "gers12"
        ); // gers12 - amount must be > 0s

        require(
            IERC20(matterTokenAddress).transferFrom(msg.sender, address(this), _repayAmount),
            "gers13"
        ); // gers13 - token transfer in failed

        totalLended -= _repayAmount;

        // Charge fees

        uint16 tokenId = validateTokenAddress(_feesTokenAddress);
        if (tokenId == 0) {
            // Token is Ether
            require(
                _feesAmount == 0 && msg.value > 0,
                "gers14"
            ); // gers14 - amount must be == 0 and msg.value > 0
            accumulatedFees += msg.value;
        } else {
            // Token is ERC20
            require(
                _feesAmount > 0 && msg.value == 0,
                "gers15"
            ); // gers15 - amount must be > 0 and msg.value == 0
            require(
                IERC20(_feesTokenAddress).transferFrom(msg.sender, address(this), _feesAmount),
                "gers16"
            ); // gers16 - token transfer in failed
            accumulatedFees += _feesAmount;
        }
    }
}