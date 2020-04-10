pragma solidity 0.5.16;

import "./Config.sol";


/// @title Governance Contract
/// @author Matter Labs
contract Governance is Config {

    /// @notice Token added to Franklin net
    event TokenAdded(
        address token,
        uint16 tokenId
    );

    /// @notice Address which will exercise governance over the network i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    /// @notice Total number of ERC20 tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    /// @notice List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    /// @notice List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    /// @notice Validator information
    struct Validator {
        uint24 id;
        bool isActive;
    }

    /// @notice List of permitted validators
    mapping(address => Validator) public validators;

    /// @notice Mapping from validator address to id
    mapping(uint24 => address) public validatorAddresses;

    /// @notice Next validator id to insert into `validators` (0 for invalid)
    uint24 nextValidatorId = 1;

    constructor() public {}

    /// @notice Governance contract initialization
    /// @param initializationParameters Encoded representation of initialization parameters:
        /// _networkGovernor The address of network governor
    function initialize(bytes calldata initializationParameters) external {
        address _networkGovernor = abi.decode(initializationParameters, (address));

        networkGovernor = _networkGovernor;

        Validator memory validator = Validator(nextValidatorId, true);
        validators[_networkGovernor] = validator;
        validatorAddresses[nextValidatorId] = _networkGovernor;

        nextValidatorId += 1;
    }

    /// @notice Change current governor
    /// @param _newGovernor Address of the new governor
    function changeGovernor(address _newGovernor) external {
        requireGovernor(msg.sender);
        networkGovernor = _newGovernor;
    }

    /// @notice Add token to the list of networks tokens
    /// @param _token Token address
    function addToken(address _token) external {
        requireGovernor(msg.sender);
        require(tokenIds[_token] == 0, "gan11"); // token exists
        require(totalTokens < MAX_AMOUNT_OF_REGISTERED_TOKENS, "gan12"); // no free identifiers for tokens

        totalTokens++;
        uint16 newTokenId = totalTokens; // it is not `totalTokens - 1` because tokenId = 0 is reserved for eth

        tokenAddresses[newTokenId] = _token;
        tokenIds[_token] = newTokenId;
        emit TokenAdded(_token, newTokenId);
    }

    /// @notice Change validator status (active or not active)
    /// @param _validatorAddress Validator address
    /// @param _active Active flag
    function setValidator(address _validatorAddress, bool _active) external {
        requireGovernor(msg.sender);

        Validator memory validator = validators[_validatorAddress];

        if (validator.id == 0) {
            validator.id = nextValidatorId;
            validatorAddresses[validator.id] = _validatorAddress;
            nextValidatorId += 1;
        }

        validator.isActive = _active;

        validators[_validatorAddress] = validator;
    }

    /// @notice Check if specified address is is governor
    /// @param _address Address to check
    function requireGovernor(address _address) public view {
        require(_address == networkGovernor, "grr11"); // only by governor
    }

    /// @notice Checks if validator is active
    /// @param _address Validator address
    function requireActiveValidator(address _address) external view {
        require(validators[_address].isActive, "grr21"); // validator is not active
    }

    /// @notice Get validator's id, checking that _address is known validator's address
    /// @param _address Validator's address
    /// @return validator's id
    function getValidatorId(address _address) external view returns (uint24) {
        uint24 validatorId = validators[_address].id;
        require(validatorId != 0, "gvi10");  // _address is not a validator's address
        return validatorId;
    }

    /// @notice Get validator's address, checking that _validatorId is known validator's id
    /// @param _validatorId Validator's id
    /// @return validator's address
    function getValidatorAddress(uint24 _validatorId) external view returns (address) {
        address validatorAddress = validatorAddresses[_validatorId];
        require(validatorAddress != address(0), "gva10");  // _validatorId is invalid
        return validatorAddress;
    }

    /// @notice Validate token id (must be less than  or equal total tokens amount)
    /// @param _tokenId Token id
    /// @return bool flag that indicates if token id is less than total tokens amount
    function isValidTokenId(uint16 _tokenId) external view returns (bool) {
        return _tokenId <= totalTokens;
    }

    /// @notice Validate token address
    /// @param _tokenAddr Token address
    /// @return tokens id
    function validateTokenAddress(address _tokenAddr) external view returns (uint16) {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(tokenId != 0, "gvs11"); // 0 is not a valid token
        return tokenId;
    }

}
