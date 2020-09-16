pragma solidity ^0.5.0;

contract TokenDeployInit {
    function getTokens() internal pure returns (address[] memory) {
        address[] memory tokens = new address[]({{ token_len }});
        {{~ #each tokens }}
        tokens[{{@index}}] = {{ this.address }};
        {{~ /each }}
        return tokens;
    }
}
