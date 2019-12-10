pragma solidity 0.5.10;

/// @title Governance Contract
/// @author Matter Labs
contract Governance {

    /// @notice Fee gas price for transactions - operators can change it depending on block generation cost
    uint256 public FEE_GAS_PRICE_MULTIPLIER = 2000000000; // 2 Gwei

    // @notice Address which will excercise governance over the network i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    /// @notice Total number of ERC20 tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    /// @notice List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    /// @notice List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    /// @notice List of permitted validators
    mapping(address => bool) public validators;

    /// @notice Token added to Franklin net
    event TokenAdded(
        address token,
        uint16 tokenId
    );

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
            "gean11"
        ); // gean11 - token exists

        // Adding one because tokenId = 0 is reserved for ETH
        tokenAddresses[totalTokens + 1] = _token;
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

    /// @notice Change validator status (active or not active)
    /// @param _address Validator address
    /// @param _active Active flag
    function setValidator(address _address, bool _active) external {
        requireGovernor();
        validators[_address] = _active;
    }

    /// @notice Check if the sender is governor
    function requireGovernor() public view {
        require(
            msg.sender == networkGovernor,
            "gerr11"
        ); // gerr11 - only by governor
    }

    /// @notice Checks if validator is active
    /// @param _address Validator address
    function requireActiveValidator(address _address) public view {
        require(
            validators[_address],
            "geir11"
        ); // geir11 - validator is not active
    }

    /// @notice Validate token id and returns its address
    /// @param _tokenId Token id
    function validateTokenId(uint16 _tokenId) external view returns (address) {
        require(
            _tokenId < totalTokens + 1,
            "gerd11"
        ); // gerd11 - unknown token id
        address tokenAddr = tokenAddresses[_tokenId];
        require(
            tokenIds[tokenAddr] == _tokenId,
             "gevd11"
        ); // geevd11 - unknown ERC20 token id
        return tokenAddr;
    }

    /// @notice Validate token address and returns its id
    /// @param _tokenAddr Token address
    function validateTokenAddress(address _tokenAddr) external view returns (uint16) {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(
            tokenAddresses[tokenId] == _tokenAddr,
            "gevs11"
        ); // gevs11 - unknown ERC20 token address
        return tokenId;
    }

    /// @notice Change fee gas price multiplier. It is used in fees calculation. Validators need to change it depending on block creation cost
    /// @param _value New fee gas price multiplier
    function changeFeeGasPriceMultiplier(uint256 _value) external {
        requireGovernor();
        FEE_GAS_PRICE_MULTIPLIER = _value;
    }

    /// @notice Returns calculated ether deposit fee in wei
    function getDepositEtherFee() external view returns (uint256) {
        return FEE_GAS_PRICE_MULTIPLIER * 179000;  // 179000 is base gas cost for deposit eth transaction
    }

    /// @notice Returns calculated erc20 deposit fee in wei
    function getDepositERC20Fee() external view returns (uint256) {
        return FEE_GAS_PRICE_MULTIPLIER * 214000;  // 214000 is base gas cost for deposit erc transaction
    }

    /// @notice Returns calculated full exit fee in wei
    function getFullExitFee() external view returns (uint256) {
        return FEE_GAS_PRICE_MULTIPLIER * 170000;  // 170000 is base gas cost for full exits transaction
    }
}