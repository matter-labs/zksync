pragma solidity ^0.7.0;

contract TokenDeployInit {
    function getTokens() internal pure returns (address[] memory) {
        address[] memory tokens = new address[](4);
        tokens[0] = 0x472d533243475f40221F3fD22C5bA5F98c71D10B;
        tokens[1] = 0x1412295a435aB93CC9082FeaB48532170189E0D2;
        tokens[2] = 0x623CFDc6F8D2011A2607254D2Fa4582691b66A45;
        tokens[3] = 0xAd4E5DE9646A905a57e26ba99d58538BcA48bf28;
        return tokens;
    }
}
