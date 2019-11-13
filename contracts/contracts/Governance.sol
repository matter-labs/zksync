pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";
import "./BlsOperations.sol";

contract Governance {

    // Token added to Franklin net
    // Structure:
    // - token - added token address
    // - tokenId - added token id
    event TokenAdded(
        address token,
        uint16 tokenId
    );

    // cToken added to Franklin net
    // Structure:
    // - cToken - added cToken address
    // - token - corresponding token address
    event cTokenAdded(
        address cToken,
        address token
    );

    // Address which will excercise governance over the network
    // i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    // Total number of ERC20 tokens registered in the network
    // (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    /// @notice validators count
    uint256 public validatorsCount;

    /// @notice Trusted addresses to withdraw funds
    mapping(address => bool) public trustedAddresses;

    /// @notice List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    /// @notice List of registered cTokens by corresponding token address
    mapping(address => address) public cTokenAddresses;

    /// @notice List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    /// @notice Validators addresses list
    mapping(uint16 => address) public validators;

    /// @notice Each validators' info
    mapping(address => ValidatorInfo) public validatorsInfo;

    /// @notice Each validators' supplies
    mapping(address => mapping(uint16 => uint256)) public validatorsBalances;

    /// @notice Funds on contract
    mapping(uint16 => uint256) public funds;
    
    /// @notice Container for information about validator
    /// @member isActive Flag for validator existance in current lending process
    /// @member id Validator id
    /// @member pubkey Validators' pubkey
    struct ValidatorInfo {
        bool isActive;
        uint16 id;
        BlsOperations.G2Point pubkey;
    }

    constructor(address _networkGovernor)
    public
    {
        networkGovernor = _networkGovernor;
    }

    /// @notice Fallback function
    function()
    external
    payable
    {
        revert("Cant accept ether in fallback function");
    }

    // Change current governor
    // _newGovernor - address of the new governor
    function changeGovernor(address _newGovernor)
    external
    {
        requireGovernor();
        networkGovernor = _newGovernor;
    }

    // Add token to the list of possible tokens
    // Params:
    // - _token - token address
    function addToken(address _token)
    external
    {
        requireGovernor();
        require(
            tokenIds[_token] == 0,
            "gan11"
        ); // gan11 - token exists
        tokenAddresses[totalTokens + 1] = _token; // Adding one because tokenId = 0 is reserved for ETH
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

    // Add cToken for token to the list of possible cTokens
    // Params:
    // - _token - token address
    function addCToken(address _cToken,
                       address _token)
    external
    {
        requireGovernor();
        require(
            tokenIds[_token] != 0,
            "gan11"
        ); // gan11 - token doenst exists
        cTokenAddresses[_token] = _cToken;
        emit cTokenAdded(_cToken, _token);
    }

    function getCTokenAddress(uint16 _tokenId)
    external
    returns (address)
    {
        address tokenAddress = tokenAddresses[_tokenId];
        return cTokenAddresses[tokenAddress];
    }

    /// @notice Change validator status
    /// @dev Only governor can add new validator
    /// @param _address Validator address
    /// @param _active Active flag
    function setValidatorStatus(address _address,
                                bool _active)
    external
    {
        requireGovernor();
        require(
            validatorsInfo[_address].pubkey.x[0] != 0 &&
            validatorsInfo[_address].pubkey.x[1] != 0 &&
            validatorsInfo[_address].pubkey.y[0] != 0 &&
            validatorsInfo[_address].pubkey.y[1] != 0,
            "ssar11"
        ); // ssar11 - operator pubkey must exist
        validatorsInfo[_address].isActive = _active;
    }

    /// @notice Add new validator with pubkey
    /// @dev Only governor can add new validator
    /// @param _address Validator address
    /// @param _pbkxx Validator pubkey xx
    /// @param _pbkxy Validator pubkey xy
    /// @param _pbkyx Validator pubkey yx
    /// @param _pbkyy Validator pubkey yy
    function addValidator(address _address,
                          uint256 _pbkxx,
                          uint256 _pbkxy,
                          uint256 _pbkyx,
                          uint256 _pbkyy)
    external
    {
        requireGovernor();
        require(
            !validatorsInfo[_address].isActive,
            "ssar11"
        ); // ssar11 - operator exists
        validatorsInfo[_address].isActive = true;
        validatorsInfo[_address].id = validatorsCount;
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
        validators[validatorsCount] = _address;
        validatorsCount++;
    }

    /// @notice Returns validators aggregated pubkey for specified validators bitmask
    /// @param _validators Validators addresses
    function getValidatorsAggrPubkey(uint16 _bitmask)
    internal
    view
    returns (BlsOperations.G2Point memory aggrPubkey,
             uint16 signersCount)
    {
        for(uint8 i = 0; i < validatorsCount; i++) {
            if( (bitmask >> i) & 1 > 0 ) {
                address addr = validators[i];
                require(
                    validatorsInfo[addr].isActive,
                    "sst011"
                ); // sst011 - not active validator
                BlsOperations.G2Point memory pubkey = validatorsInfo[addr].pubkey;
                aggrPubkey = BlsOperations.addG2(aggrPubkey, pubkey);
                signersCount++;
            }
        }
    }

    function verifySenderAndBlsSignature(address _sender,
                                         uint256 _aggrSignatureX,
                                         uint256 _aggrSignatureY,
                                         uint256 _signersBitmask,
                                         uint256 _messageHash)
    external
    view
    returns (bool result)
    {
        uint16 validatorId = validatorsInfo[_sender].id;
        require(
            (_signersBitmask >> validatorId) & 1 > 0,
            "sst011"
        ); // sst011 - sender is not in validators bitmask
        BlsOperations.G1Point memory signature = BlsOperations.G1Point(_aggrSignatureX, _aggrSignatureY);
        BlsOperations.G1Point memory mpoint = BlsOperations.messageHashToG1(_messageHash);
        (BlsOperations.G2Point memory aggrPubkey, uint16 signersCount) = getValidatorsAggrPubkey(_signersBitmask);
        require(
            signersCount >= 2 * validatorsCount / 3,
            "sst011"
        ); // sst011 - not enouth validators count
        return BlsOperations.pairing(mpoint, aggrPubkey, signature, BlsOperations.negate(BlsOperations.generatorG2()));
    }

    // Validate token address
    function validateTokenAddress(address _tokenAddr)
    public
    view
    returns (uint16)
    {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(
            tokenAddresses[tokenId] == _tokenAddr,
            "gvs11"
        ); // gvs11 - unknown ERC20 token address
        return tokenId;
    }

    // Validate token id
    function validateTokenId(uint16 _tokenId)
    public
    view
    returns (address)
    {
        uint16 tokenAddr = tokenAddresses[_tokenId];
        require(
            tokenIds[tokenAddr] == _tokenId,
            "gvs11"
        ); // gvs11 - unknown ERC20 token id
        return tokenAddr;
    }

    // Check if the sender is governor
    function requireGovernor()
    public
    view
    {
        require(
            msg.sender == networkGovernor,
            "grr11"
        ); // grr11 - only by governor
    }
    
    /// @notice Gets allowed withdraw amount for validator
    /// @dev Requires validators' existance
    /// @param _address Validator address
    /// @param _tokenId Token id
    function getAllowedWithdrawAmount(address _address,
                                      uint16 _tokenId)
    public
    returns (uint256)
    {
        uint256 supply = funds[_tokenId];
        uint256 balance = validatorsBalances[_address][_tokenId];
        if (supply >= balance) {
            return balance;
        } else {
            return supply;
        }
    }

    /// @notice Withdraws specified amount from validators supply
    /// @dev Requires validators' existance and allowed amount is >= specified amount, which should not be equal to 0
    /// @param _tokenAddress Token address
    /// @param _amount Specified amount
    function withdraw(address _tokenAddress,
                      uint256 _amount)
    external
    {
        require(
            _amount > 0,
            "ssir11"
        ); // ssir11 - amount must be > 0
        address tokenId = validateTokenAddress(_tokenAddress);
        require(
            getAllowedWithdrawAmount(msg.sender, tokenId) >= _amount,
            "sswr11"
        ); // sswr11 - wrong amount - higher than allowed
        if (tokenId == 0) {
            msg.sender.transfer(_amount); // transfer ether
        } else {
            require(
                IERC20(_tokenAddress).transfer(msg.sender, _amount),
                "sstt11"
            ); // sstt11 - token transfer out failed
        }
        funds[tokenId] -= _amount;
        validatorsBalances[msg.sender][tokenId] -= _amount;
    }
    /// @notice Supplies specified amount of ERC20 tokens from validator
    /// @dev Calls transferIn function of specified token and fulfillDefferedWithdrawOrders to fulfill deffered withdraw orders
    /// @param _tokenAddress Token address
    /// @param _amount Token amount
    function supplyErc20(uint16 _tokenId,
                         uint256 _amount,
                         address _validator)
    external
    {
        address tokenAddress = validateTokenId(_tokenId);
        require(
            IERC20(tokenAddress).transferFrom(msg.sender, address(this), _amount),
            "sst011"
        ); // sst011 - token transfer in failed
        funds[tokenId] += _amount;
        validatorsBalances[_validator][tokenId] += _amount;
    }

    /// @notice Function to accept ether payments
    function supplyEther(address _validator)
    external
    payable
    {
        funds[0] += msg.value;
        validatorsBalances[_validator][0] += _amount;
    }

    function setTrustedAddress(address _address)
    external
    {
        requireGovernor();
        trustedAddresses[_address] = true;
    }

    function borrowToTrustedAddress(uint16 _tokenId,
                                    uint256 _amount)
    external
    returns(uint16 suppliersCount)
    {
        require(
            trustedAddresses[msg.sender],
            "grr11"
        ); // grr11 - not trusted address

        address tokenAddress = validateTokenId(_tokenId);

        uint256 supply = funds[_tokenId];
        if (supply < _amount) {
            return 0;
        }
        if (_tokenId == 0) {
            msg.sender.transfer(_amount);
        } else {
            require(
                IERC20(tokenAddress).transfer(swiftExits, _amount),
                "sstt11"
            ); // sstt11 - token transfer out failed
        }

        for(uint16 i = 0; i < validatorsCount; i++) {
            validatorsBalances[validators[i]][_tokenId] -= _amount * validatorsBalances[validators[i]][_tokenId] / funds[_tokenId];
        }
        funds[_tokenId] -= _amount;
        return validatorsCount;
    }

    /// @notice Transfers specified amount of ERC20 token into contract
    /// @param _tokenAddress ERC20 token address
    /// @param _amount Amount in ERC20 tokens
    function repayInErc20(uint16 _tokenId,
                          uint256 _amount,
                          uint16 _validatorsCount,
                          uint16 _excludedValidatorsBitmask)
    external
    {
        require(
            trustedAddresses[msg.sender],
            "grr11"
        ); // grr11 - not trusted address

        address tokenAddress = validateTokenId(_tokenId);
        require(
            IERC20(tokenAddress).transferFrom(msg.sender, address(this), _amount),
            "sst011"
        ); // sst011 - token transfer in failed

        for(uint8 i = 0; i < _validatorsCount; i++) {
            if( (_excludedValidatorsBitmask >> i) & 1 == 0 ) {
                validatorsBalances[validators[i]][_tokenId] += _amount * validatorsBalances[validators[i]][_tokenId] / funds[_tokenId];
            }
        }
        funds[_tokenId] += _amount;
    }

    /// @notice Transfers specified amount of ERC20 token into contract
    /// @param _amount Amount in ERC20 tokens
    function repayInEther(uint16 _validatorsCount,
                          uint16 _excludedValidatorsBitmask)
    external
    payable
    {
        require(
            trustedAddresses[msg.sender],
            "grr11"
        ); // grr11 - not trusted address

        for(uint8 i = 0; i < _validatorsCount; i++) {
            if( (_excludedValidatorsBitmask >> i) & 1 == 0 ) {
                validatorsBalances[validators[i]][0] += _amount * validatorsBalances[validators[i]][0] / funds[0];
            }
        }
        funds[0] += _amount;
    }
}