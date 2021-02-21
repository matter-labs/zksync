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
    event TokenListerUpdate(address, bool);

    /// @notice zkSync governance contract
    Governance public governance;

    /// @notice Token used to collect listing payment for addition of new token to zkSync network
    IERC20 public listingPriceToken;

    /// @notice Size of the listing payment
    uint256 public listingPrice;

    /// @notice Max number of tokens that can be listed using this contract
    uint16 public maxToken;

    /// @notice Addresses that can list tokens without fee
    mapping(address => bool) public tokenLister;

    /// @notice Address that collects listing payments
    address public treasury;

    constructor(
        Governance _governance,
        IERC20 _listingPriceToken,
        uint256 _listingPrice,
        uint16 _maxToken,
        address _treasury
    ) {
        governance = _governance;
        listingPriceToken = _listingPriceToken;
        listingPrice = _listingPrice;
        maxToken = _maxToken;
        treasury = _treasury;

        // We add zkSync governor as a first token lister.
        tokenLister[governance.networkGovernor()] = true;
        emit TokenListerUpdate(governance.networkGovernor(), true);
    }

    /// @notice Adds new ERC20 token to zkSync network.
    /// @notice If caller is not present in the `tokenLister` map payment of `listingPrice` in `listingPriceToken` should be made.
    /// @notice NOTE: before calling this function make sure to approve `listingPriceToken` transfer for this contract.
    function addToken(address _token) external {
        require(governance.totalTokens() < maxToken, "can't add more tokens"); // Impossible to add more tokens using this contract
        if (!tokenLister[msg.sender]) {
            // Collect fees
            bool feeTransferOk = Utils.transferFromERC20(listingPriceToken, msg.sender, treasury, listingPrice);
            require(feeTransferOk, "fee transfer failed"); // Failed to receive payment for token addition.
        }
        governance.addToken(_token);
    }

    /// Governance functions (this contract is governed by zkSync governor)

    /// @notice Set new listing token and price
    /// @notice Can be called only by zkSync governor
    function setListingToken(IERC20 _newListingToken, uint256 _newListingPrice) external {
        governance.requireGovernor(msg.sender);
        listingPriceToken = _newListingToken;
        listingPrice = _newListingPrice;
    }

    /// @notice Set new listing price
    /// @notice Can be called only by zkSync governor
    function setListingPrice(uint256 _newListingPrice) external {
        governance.requireGovernor(msg.sender);
        listingPrice = _newListingPrice;
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
        maxToken = _newListingCap;
    }

    /// @notice Change address that collects payments for listing tokens.
    /// @notice Can be called only by zkSync governor
    function setTreasury(address _newTreasury) external {
        governance.requireGovernor(msg.sender);
        treasury = _newTreasury;
    }
}
