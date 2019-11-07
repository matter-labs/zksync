pragma solidity ^0.5.8;

import "./PriceOracle.sol";
import "./CErc20.sol";

contract SimplePriceOracle is PriceOracle {
    mapping(address => uint) prices;
    bool public constant isPriceOracle = true;

    function getUnderlyingPrice(CToken cToken) public view returns (uint) {
        return prices[address(CErc20(address(cToken)).underlying())];
    }

    function setUnderlyingPrice(CToken cToken, uint underlyingPriceMantissa) public {
        prices[address(CErc20(address(cToken)).underlying())] = underlyingPriceMantissa;
    }

    function setDirectPrice(address a, uint price) public {
        prices[a] = price;
    }

    // v1 price oracle interface for use as backing of proxy
    function assetPrices(address asset) external view returns (uint) {
        return prices[asset];
    }
}
