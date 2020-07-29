pragma solidity ^0.5.0;

import "./Context.sol";
import "./IERC20.sol";
import "./SafeMath.sol";
import "./ZkSync.sol";

contract RedditToken is Context, IERC20 {
    using SafeMath for uint256;

    mapping (address => uint256) private _balances;

    mapping (address => mapping (address => uint256)) private _allowances;

    uint256 private _totalSupply;

    ZkSync ZkSyncContract;

    uint104 constant INITIAL_AMOUNT_TO_DEPOSIT_TO_ZKSYNC = 10**20;

    constructor(address _ZkSyncAddress) public {
        ZkSyncContract = ZkSync(_ZkSyncAddress);
        ZkSyncContract.depositERC20(IERC20(this), INITIAL_AMOUNT_TO_DEPOSIT_TO_ZKSYNC, address(this));
    }

    function totalSupply() public view returns (uint256) {
        return _totalSupply;
    }

    function balanceOf(address account) public view returns (uint256) {
        return _balances[account];
    }

    function transfer(address recipient, uint256 amount) public returns (bool) {
        _transfer(_msgSender(), recipient, amount);
        return true;
    }

    function allowance(address owner, address spender) public view returns (uint256) {
        return _allowances[owner][spender];
    }

    function approve(address spender, uint256 amount) public returns (bool) {
        _approve(_msgSender(), spender, amount);
        return true;
    }

    function transferFrom(address sender, address recipient, uint256 amount) public returns (bool) {
        _transfer(sender, recipient, amount);
        _approve(sender, _msgSender(), _allowances[sender][_msgSender()].sub(amount, "ERC20: transfer amount exceeds allowance"));
        return true;
    }

    function increaseAllowance(address spender, uint256 addedValue) public returns (bool) {
        _approve(_msgSender(), spender, _allowances[_msgSender()][spender].add(addedValue));
        return true;
    }

    function decreaseAllowance(address spender, uint256 subtractedValue) public returns (bool) {
        _approve(_msgSender(), spender, _allowances[_msgSender()][spender].sub(subtractedValue, "ERC20: decreased allowance below zero"));
        return true;
    }

    function _transfer(address sender, address recipient, uint256 amount) internal {
        require(sender != address(0), "ERC20: transfer from the zero address");
        require(recipient != address(0), "ERC20: transfer to the zero address");

        if (sender == address(ZkSyncContract)) {
            _totalSupply = _totalSupply.add(amount);
        } else {
            _balances[sender] = _balances[sender].sub(amount, "ERC20: transfer amount exceeds balance");
        }

        if (recipient == address(ZkSyncContract)){
            _totalSupply = _totalSupply.sub(amount);
        } else{
            _balances[recipient] = _balances[recipient].add(amount);
        }

        emit Transfer(sender, recipient, amount);
    }

    function _approve(address owner, address spender, uint256 amount) internal {
        require(owner != address(0), "ERC20: approve from the zero address");
        require(spender != address(0), "ERC20: approve to the zero address");

        _allowances[owner][spender] = amount;
        emit Approval(owner, spender, amount);
    }

    function setMintingMultisigKey(bytes calldata _newPubKeyHash, uint32 _nonce) external {
        /* TODO: check authorization here */

        ZkSyncContract.setAuthPubkeyHash(_newPubKeyHash, _nonce);
    }
}
