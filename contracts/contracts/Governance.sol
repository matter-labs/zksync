pragma solidity ^0.5.8;

contract Governance {

    // Token added to Franklin net
    // Structure:
    // - token - added token address
    // - tokenId - added token id
    event TokenAdded(
        address token,
        uint16 tokenId
    );

    // Address which will excercise governance over the network
    // i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    // Total number of ERC20 tokens registered in the network
    // (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    // List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    // List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    // List of permitted validators
    mapping(address => bool) public validators;

    constructor(address _networkGovernor) public {
        networkGovernor = _networkGovernor;
        validators[_networkGovernor] = true;
    }

    // Change current governor
    // _newGovernor - address of the new governor
    function changeGovernor(address _newGovernor) external {
        requireGovernor();
        networkGovernor = _newGovernor;
    }

    // Add token to the list of possible tokens
    // Params:
    // - _token - token address
    function addToken(address _token) external {
        requireGovernor();
        require(tokenIds[_token] == 0, "token exists");
        tokenAddresses[totalTokens + 1] = _token; // Adding one because tokenId = 0 is reserved for ETH
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

    // Set validator status
    // Params:
    // - _validator - validator address
    // - _active - bool value (true if validator is active)
    function setValidator(address _validator, bool _active) external {
        requireGovernor();
        validators[_validator] = _active;
    }

    // Check if the sender is governor
    function requireGovernor() internal view {
        require(msg.sender == networkGovernor, "only by governor");
    }

    // Check if sender is validator
    function requireValidator(address sender) external view {
        require(validators[sender], "only by validator");
    }

    // Check if token is known
    function requireValidTokenId(uint16 _tokenId) external view {
        require(_tokenId < totalTokens + 1, "unknown token");
    }

    // Validate token address
    function validateERC20Token(address tokenAddr) external view returns (uint16) {
        uint16 tokenId = tokenIds[tokenAddr];
        require(tokenAddresses[tokenId] == tokenAddr, "unknown ERC20 token");
        return tokenId;
    }

}