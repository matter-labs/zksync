pragma solidity 0.5.10;

contract Governance {

    // Fee gas price for transactions - operators can change it depending on block generation cost
    uint256 public FEE_GAS_PRICE_MULTIPLIER = 2000000000; // 2 Gwei

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
        require(
            tokenIds[_token] == 0,
            "gan11"
        ); // gan11 - token exists
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
        require(
            msg.sender == networkGovernor,
            "grr11"
        ); // grr11 - only by governor
    }

    // Check if sender is validator
    function isValidator(address _sender) external view returns (bool) {
        return validators[_sender];
    }

    // Check if token is known
    function requireValidTokenId(uint16 _tokenId) external view {
        require(
            _tokenId < totalTokens + 1,
            "grd11"
        ); // grd11 - unknown token id
    }

    // Validate token address
    function validateTokenAddress(address _tokenAddr) external view returns (uint16) {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(
            tokenAddresses[tokenId] == _tokenAddr,
            "gvs11"
        ); // gvs11 - unknown ERC20 token address
        return tokenId;
    }

    function changeFeeGasPriceMultiplier(uint256 _value) external {
        requireGovernor();
        FEE_GAS_PRICE_MULTIPLIER = _value;
    }

    function getDepositEtherFee() external returns (uint256) {
        return FEE_GAS_PRICE_MULTIPLIER * 179000;  // 179000 is base gas cost for deposit eth transaction
    }

    function getDepositERC20Fee() external returns (uint256) {
        return FEE_GAS_PRICE_MULTIPLIER * 214000;  // 214000 is base gas cost for deposit erc transaction
    }

    function getFullExitFee() external returns (uint256) {
        return FEE_GAS_PRICE_MULTIPLIER * 170000;  // 170000 is base gas cost for full exits transaction
    }
}