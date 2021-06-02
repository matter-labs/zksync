// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./Config.sol";
import "./Utils.sol";
import "./NFTFactory.sol";
import "./TokenGovernance.sol";

/// @title Governance Contract
/// @author Matter Labs
contract Governance is Config {
    /// @notice Token added to Franklin net
    event NewToken(address indexed token, uint16 indexed tokenId);

    /// @notice Default nft factory has set
    event SetDefaultNFTFactory(address indexed factory);

    /// @notice NFT factory registered new creator account
    event NFTFactoryRegisteredCreator(
        uint32 indexed creatorAccountId,
        address indexed creatorAddress,
        address factoryAddress
    );

    /// @notice Governor changed
    event NewGovernor(address newGovernor);

    /// @notice Token Governance changed
    event NewTokenGovernance(TokenGovernance newTokenGovernance);

    /// @notice Validator's status changed
    event ValidatorStatusUpdate(address indexed validatorAddress, bool isActive);

    event TokenPausedUpdate(address indexed token, bool paused);

    /// @notice Address which will exercise governance over the network i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    /// @notice Total number of ERC20 tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 public totalTokens;

    /// @notice List of registered tokens by tokenId
    mapping(uint16 => address) public tokenAddresses;

    /// @notice List of registered tokens by address
    mapping(address => uint16) public tokenIds;

    /// @notice List of permitted validators
    mapping(address => bool) public validators;

    /// @notice Paused tokens list, deposits are impossible to create for paused tokens
    mapping(uint16 => bool) public pausedTokens;

    /// @notice Address that is authorized to add tokens to the Governance.
    TokenGovernance public tokenGovernance;

    /// @notice NFT Creator address to factory address mapping
    mapping(uint32 => mapping(address => NFTFactory)) public nftFactories;

    /// @notice Address which will be used if NFT token has no factories
    NFTFactory public defaultFactory;

    /// @notice Governance contract initialization. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param initializationParameters Encoded representation of initialization parameters:
    ///     _networkGovernor The address of network governor
    function initialize(bytes calldata initializationParameters) external {
        address _networkGovernor = abi.decode(initializationParameters, (address));

        networkGovernor = _networkGovernor;
    }

    /// @notice Governance contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    // solhint-disable-next-line no-empty-blocks
    function upgrade(bytes calldata upgradeParameters) external {}

    /// @notice Change current governor
    /// @param _newGovernor Address of the new governor
    function changeGovernor(address _newGovernor) external {
        requireGovernor(msg.sender);
        if (networkGovernor != _newGovernor) {
            networkGovernor = _newGovernor;
            emit NewGovernor(_newGovernor);
        }
    }

    /// @notice Change current token governance
    /// @param _newTokenGovernance Address of the new token governor
    function changeTokenGovernance(TokenGovernance _newTokenGovernance) external {
        requireGovernor(msg.sender);
        if (tokenGovernance != _newTokenGovernance) {
            tokenGovernance = _newTokenGovernance;
            emit NewTokenGovernance(_newTokenGovernance);
        }
    }

    /// @notice Add token to the list of networks tokens
    /// @param _token Token address
    function addToken(address _token) external {
        require(msg.sender == address(tokenGovernance), "1E");
        require(tokenIds[_token] == 0, "1e"); // token exists
        require(totalTokens < MAX_AMOUNT_OF_REGISTERED_TOKENS, "1f"); // no free identifiers for tokens

        totalTokens++;
        uint16 newTokenId = totalTokens; // it is not `totalTokens - 1` because tokenId = 0 is reserved for eth

        tokenAddresses[newTokenId] = _token;
        tokenIds[_token] = newTokenId;
        emit NewToken(_token, newTokenId);
    }

    /// @notice Pause token deposits for the given token
    /// @param _tokenAddr Token address
    /// @param _tokenPaused Token paused status
    function setTokenPaused(address _tokenAddr, bool _tokenPaused) external {
        requireGovernor(msg.sender);

        uint16 tokenId = this.validateTokenAddress(_tokenAddr);
        if (pausedTokens[tokenId] != _tokenPaused) {
            pausedTokens[tokenId] = _tokenPaused;
            emit TokenPausedUpdate(_tokenAddr, _tokenPaused);
        }
    }

    /// @notice Change validator status (active or not active)
    /// @param _validator Validator address
    /// @param _active Active flag
    function setValidator(address _validator, bool _active) external {
        requireGovernor(msg.sender);
        if (validators[_validator] != _active) {
            validators[_validator] = _active;
            emit ValidatorStatusUpdate(_validator, _active);
        }
    }

    /// @notice Check if specified address is is governor
    /// @param _address Address to check
    function requireGovernor(address _address) public view {
        require(_address == networkGovernor, "1g"); // only by governor
    }

    /// @notice Checks if validator is active
    /// @param _address Validator address
    function requireActiveValidator(address _address) external view {
        require(validators[_address], "1h"); // validator is not active
    }

    /// @notice Validate token id (must be less than or equal to total tokens amount)
    /// @param _tokenId Token id
    /// @return bool flag that indicates if token id is less than or equal to total tokens amount
    function isValidTokenId(uint16 _tokenId) external view returns (bool) {
        return _tokenId <= totalTokens;
    }

    /// @notice Validate token address
    /// @param _tokenAddr Token address
    /// @return tokens id
    function validateTokenAddress(address _tokenAddr) external view returns (uint16) {
        uint16 tokenId = tokenIds[_tokenAddr];
        require(tokenId != 0, "1i"); // 0 is not a valid token
        return tokenId;
    }

    function packRegisterNFTFactoryMsg(
        uint32 _creatorAccountId,
        address _creatorAddress,
        address _factoryAddress
    ) internal pure returns (bytes memory) {
        return
            abi.encodePacked(
                "\x19Ethereum Signed Message:\n141",
                "\nCreator's account ID in zkSync: ",
                Bytes.bytesToHexASCIIBytes(abi.encodePacked((_creatorAccountId))),
                "\nCreator: ",
                Bytes.bytesToHexASCIIBytes(abi.encodePacked((_creatorAddress))),
                "\nFactory: ",
                Bytes.bytesToHexASCIIBytes(abi.encodePacked((_factoryAddress)))
            );
    }

    /// @notice Register creator corresponding to the factory
    /// @param _creatorAccountId Creator's zkSync account ID
    /// @param _creatorAddress NFT creator address
    /// @param _signature Creator's signature
    function registerNFTFactoryCreator(
        uint32 _creatorAccountId,
        address _creatorAddress,
        bytes memory _signature
    ) external {
        require(address(nftFactories[_creatorAccountId][_creatorAddress]) == address(0), "Q");
        bytes32 messageHash = keccak256(packRegisterNFTFactoryMsg(_creatorAccountId, _creatorAddress, msg.sender));

        address recoveredAddress = Utils.recoverAddressFromEthSignature(_signature, messageHash);
        require(recoveredAddress == _creatorAddress && recoveredAddress != address(0), "ws");
        nftFactories[_creatorAccountId][_creatorAddress] = NFTFactory(msg.sender);
        emit NFTFactoryRegisteredCreator(_creatorAccountId, _creatorAddress, msg.sender);
    }

    //@notice Set default factory for our contract. This factory will be used to mint an NFT token that has no factory
    //@param _factory Address of NFT factory
    function setDefaultNFTFactory(address _factory) external {
        requireGovernor(msg.sender);
        require(address(_factory) != address(0), "mb1"); // Factory should be non zero
        require(address(defaultFactory) == address(0), "mb2"); // NFTFactory is already set
        defaultFactory = NFTFactory(_factory);
        emit SetDefaultNFTFactory(_factory);
    }

    function getNFTFactory(uint32 _creatorAccountId, address _creatorAddress) external view returns (NFTFactory) {
        NFTFactory _factory = nftFactories[_creatorAccountId][_creatorAddress];
        if (address(_factory) == address(0)) {
            require(address(defaultFactory) != address(0), "fs"); // NFTFactory does not set
            return defaultFactory;
        } else {
            return _factory;
        }
    }
}
