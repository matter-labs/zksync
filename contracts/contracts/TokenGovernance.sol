// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./Governance.sol";
import "./IERC20.sol";
import "./Utils.sol";

/// @title Token Governance Contract
/// @author Matter Labs
/// @notice Contract is used to allow anyone to add new ERC20 tokens to zkSync given sufficient payment
contract TokenGovernance {
    /// @notice Token lister added or removed (see `tokenLister`)
    event TokenListerUpdate(address indexed tokenLister, bool isActive);

    /// @notice Listing fee token set
    event ListingFeeTokenUpdate(IERC20 indexed newListingFeeToken);

    /// @notice Listing fee set
    event ListingFeeUpdate(uint256 newListingFee);

    /// @notice Maximum number of listed tokens updated
    event ListingCapUpdate(uint16 newListingCap);

    /// @notice The treasury (the account which will receive the fee) was updated
    event TreasuryUpdate(address newTreasury);

    /// @notice zkSync governance contract
    Governance public governance;

    /// @notice Token used to collect listing fee for addition of new token to zkSync network
    IERC20 public listingFeeToken;

    /// @notice Token listing fee
    uint256 public listingFee;

    /// @notice Max number of tokens that can be listed using this contract
    uint16 public listingCap;

    /// @notice Addresses that can list tokens without fee
    mapping(address => bool) public tokenLister;

    /// @notice Address that collects listing payments
    address public treasury;

    constructor(
        Governance _governance,
        IERC20 _listingFeeToken,
        uint256 _listingFee,
        uint16 _listingCap,
        address _treasury
    ) {
        governance = _governance;
        listingFeeToken = _listingFeeToken;
        listingFee = _listingFee;
        listingCap = _listingCap;
        treasury = _treasury;

        address governor = governance.networkGovernor();
        // We add zkSync governor as a first token lister.
        tokenLister[governor] = true;
        emit TokenListerUpdate(governor, true);
    }

    /// @notice Adds new ERC20 token to zkSync network.
    /// @notice If caller is not present in the `tokenLister` map payment of `listingFee` in `listingFeeToken` should be made.
    /// @notice NOTE: before calling this function make sure to approve `listingFeeToken` transfer for this contract.
    function addToken(address _token) external {
        require(governance.totalTokens() < listingCap, "can't add more tokens"); // Impossible to add more tokens using this contract
        if (!tokenLister[msg.sender]) {
            // Collect fees
            bool feeTransferOk = Utils.transferFromERC20(listingFeeToken, msg.sender, treasury, listingFee);
            require(feeTransferOk, "fee transfer failed"); // Failed to receive payment for token addition.
        }
        governance.addToken(_token);
    }

    /// Governance functions (this contract is governed by zkSync governor)

    /// @notice Set new listing token and fee
    /// @notice Can be called only by zkSync governor
    function setListingFeeToken(IERC20 _newListingFeeToken, uint256 _newListingFee) external {
        governance.requireGovernor(msg.sender);
        listingFeeToken = _newListingFeeToken;
        listingFee = _newListingFee;

        emit ListingFeeTokenUpdate(_newListingFeeToken);
    }

    /// @notice Set new listing fee
    /// @notice Can be called only by zkSync governor
    function setListingFee(uint256 _newListingFee) external {
        governance.requireGovernor(msg.sender);
        listingFee = _newListingFee;

        emit ListingFeeUpdate(_newListingFee);
    }

    /// @notice Enable or disable token lister. If enabled new tokens can be added by that address without payment
    /// @notice Can be called only by zkSync governor
    function setLister(address _listerAddress, bool _active) external {
        governance.requireGovernor(msg.sender);
        if (tokenLister[_listerAddress] != _active) {
            tokenLister[_listerAddress] = _active;
            emit TokenListerUpdate(_listerAddress, _active);
        }
    }

    /// @notice Change maximum amount of tokens that can be listed using this method
    /// @notice Can be called only by zkSync governor
    function setListingCap(uint16 _newListingCap) external {
        governance.requireGovernor(msg.sender);
        listingCap = _newListingCap;

        emit ListingCapUpdate(_newListingCap);
    }

    /// @notice Change address that collects payments for listing tokens.
    /// @notice Can be called only by zkSync governor
    function setTreasury(address _newTreasury) external {
        governance.requireGovernor(msg.sender);
        treasury = _newTreasury;

        emit TreasuryUpdate(_newTreasury);
    }
}
