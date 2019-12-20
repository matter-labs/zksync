pragma solidity 0.5.10;

/// @title Governance Contract
/// @author Matter Labs
contract Governance {

    /// @notice Token added to Franklin net
    event TokenAdded(
        address token,
        uint16 tokenId
    );

    /// @notice Address which will excercise governance over the network i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    /// @notice Total number of ERC20 tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    /// @notice List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    /// @notice List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    /// @notice List of permitted validators
    mapping(address => bool) public validators;

    /// @notice Construct Governance contract
    /// @param _networkGovernor The address of network governor
    constructor(address _networkGovernor) public {
        networkGovernor = _networkGovernor;
        validators[_networkGovernor] = true;
    }

    /// @notice Change current governor
    /// @param _newGovernor Address of the new governor
    function changeGovernor(address _newGovernor) external {
        requireGovernor();
        networkGovernor = _newGovernor;
    }

    /// @notice Add token to the list of networks tokens
    /// @param _token Token address
    function addToken(address _token) external {
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

    /// @notice Change validator status (active or not active)
    /// @param _validator Validator address
    /// @param _active Active flag
    function setValidator(address _validator, bool _active) external {
        requireGovernor();
        validators[_validator] = _active;
    }

    /// @notice Check if the sender is governor
    function requireGovernor() internal view {
        require(
            msg.sender == networkGovernor,
            "grr11"
        ); // grr11 - only by governor
    }

    /// @notice Return validator status (active or not)
    /// @param _sender Validator address
    /// @return bool flag that indicates validator status
    function isValidator(address _sender) external view returns (bool) {
        return validators[_sender];
    }

    /// @notice Validate token id (must be less than total tokens amount)
    /// @param _tokenId Token id
    function requireValidTokenId(uint16 _tokenId) external view {
        require(
            _tokenId < totalTokens + 1,
            "grd11"
        ); // grd11 - unknown token id
    }

    /// @notice Validate token id (must be less than total tokens amount)
    /// @param _tokenId Token id
    /// @return bool flag that indicates if token id is less than total tokens amount
    function isValidTokenId(uint16 _tokenId) external view returns (bool) {
        return _tokenId < totalTokens + 1;
    }

    /// @notice Validate token address
    /// @param _tokenAddr Token address
    /// @return tokens id
    function validateTokenAddress(address _tokenAddr) external view returns (uint16) {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(
            tokenAddresses[tokenId] == _tokenAddr,
            "gvs11"
        ); // gvs11 - unknown ERC20 token address
        return tokenId;
    }

}