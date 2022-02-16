pragma solidity ^0.7.0;

contract Create2Factory {
    /**
     * @dev Deploys a contract using `CREATE2`. The address where the contract
     * will be deployed can be known in advance via {computeAddress}. Note that
     * a contract cannot be deployed twice using the same salt.
     */
    function deploy(bytes32 salt, bytes memory bytecode) public returns (address) {
        address addr;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            addr := create2(0, add(bytecode, 0x20), mload(bytecode), salt)
        }
        require(addr != address(0), "Create2: Failed on deploy");
        return addr;
    }

    /**
     * @dev Returns the address where a contract will be stored if deployed via {deploy}. Any change in the `bytecode`
     * or `salt` will result in a new destination address.
     */
    function computeAddress(bytes32 salt, bytes memory bytecode) external view returns (address) {
        return computeAddress(salt, bytecode, address(this));
    }

    /**
     * @dev Returns the address where a contract will be stored if deployed via {deploy} from a contract located at
     * `deployer`. If `deployer` is this contract's address, returns the same value as {computeAddress}.
     */
    function computeAddress(
        bytes32 salt,
        bytes memory bytecode,
        address deployer
    ) public pure returns (address) {
        bytes32 bytecodeHash = keccak256(bytecode);
        bytes32 _data = keccak256(abi.encodePacked(bytes1(0xff), deployer, salt, bytecodeHash));
        return address(bytes20(_data << 96));
    }
}
