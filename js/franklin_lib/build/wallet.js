"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : new P(function (resolve) { resolve(result.value); }).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __generator = (this && this.__generator) || function (thisArg, body) {
    var _ = { label: 0, sent: function() { if (t[0] & 1) throw t[1]; return t[1]; }, trys: [], ops: [] }, f, y, t, g;
    return g = { next: verb(0), "throw": verb(1), "return": verb(2) }, typeof Symbol === "function" && (g[Symbol.iterator] = function() { return this; }), g;
    function verb(n) { return function (v) { return step([n, v]); }; }
    function step(op) {
        if (f) throw new TypeError("Generator is already executing.");
        while (_) try {
            if (f = 1, y && (t = op[0] & 2 ? y["return"] : op[0] ? y["throw"] || ((t = y["return"]) && t.call(y), 0) : y.next) && !(t = t.call(y, op[1])).done) return t;
            if (y = 0, t) op = [op[0] & 2, t.value];
            switch (op[0]) {
                case 0: case 1: t = op; break;
                case 4: _.label++; return { value: op[1], done: false };
                case 5: _.label++; y = op[1]; op = [0]; continue;
                case 7: op = _.ops.pop(); _.trys.pop(); continue;
                default:
                    if (!(t = _.trys, t = t.length > 0 && t[t.length - 1]) && (op[0] === 6 || op[0] === 2)) { _ = 0; continue; }
                    if (op[0] === 3 && (!t || (op[1] > t[0] && op[1] < t[3]))) { _.label = op[1]; break; }
                    if (op[0] === 6 && _.label < t[1]) { _.label = t[1]; t = op; break; }
                    if (t && _.label < t[2]) { _.label = t[2]; _.ops.push(op); break; }
                    if (t[2]) _.ops.pop();
                    _.trys.pop(); continue;
            }
            op = body.call(thisArg, _);
        } catch (e) { op = [6, e]; y = 0; } finally { f = t = 0; }
        if (op[0] & 5) throw op[1]; return { value: op[0] ? op[1] : void 0, done: true };
    }
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
var _this = this;
Object.defineProperty(exports, "__esModule", { value: true });
var BN = require("bn.js");
var axios_1 = __importDefault(require("axios"));
var crypto_1 = require("./crypto");
var ethers_1 = require("ethers");
var utils_1 = require("./utils");
var parseEther = ethers_1.ethers.utils.parseEther;
var bigNumberify = ethers_1.ethers.utils.bigNumberify;
var PUBKEY_HASH_LEN = 20;
var IERC20Conract = require("../abi/IERC20.json");
var franklinContractCode = require("../abi/Franklin.json");
function toAddress(addressLike) {
    if (typeof (addressLike) == "string") {
        return Buffer.from(addressLike.substr(2), "hex");
    }
    else {
        return addressLike;
    }
}
exports.toAddress = toAddress;
var sleep = function (ms) { return __awaiter(_this, void 0, void 0, function () { return __generator(this, function (_a) {
    switch (_a.label) {
        case 0: return [4 /*yield*/, new Promise(function (resolve) { return setTimeout(resolve, ms); })];
        case 1: return [2 /*return*/, _a.sent()];
    }
}); }); };
var FranklinProvider = /** @class */ (function () {
    function FranklinProvider(providerAddress, contractAddress) {
        if (providerAddress === void 0) { providerAddress = 'http://127.0.0.1:3000'; }
        if (contractAddress === void 0) { contractAddress = process.env.CONTRACT_ADDR; }
        this.providerAddress = providerAddress;
        this.contractAddress = contractAddress;
    }
    FranklinProvider.prepareTransferRequestForNode = function (tx, signature) {
        var req = tx;
        req.type = "Transfer";
        req.from = "0x" + tx.from.toString("hex");
        req.to = "0x" + tx.to.toString("hex");
        req.amount = bigNumberify(tx.amount).toString();
        req.fee = bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    };
    FranklinProvider.prepareWithdrawRequestForNode = function (tx, signature) {
        var req = tx;
        req.type = "Withdraw";
        req.account = "0x" + tx.account.toString("hex");
        req.amount = bigNumberify(tx.amount).toString();
        req.fee = bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    };
    FranklinProvider.prepareCloseRequestForNode = function (tx, signature) {
        var req = tx;
        req.type = "Close";
        req.account = "0x" + tx.account.toString("hex");
        req.signature = signature;
        return req;
    };
    // TODO: reconsider when wallet refactor.
    FranklinProvider.axiosRequest = function (promise) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        promise = promise
                            .then(function (reps) { return reps.data; })
                            .catch(function (error) {
                            var response;
                            if (!error.response) {
                                response = 'Error: Network Error';
                            }
                            else {
                                response = error.response.data.message;
                            }
                            throw new Error(response);
                        });
                        return [4 /*yield*/, promise];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.submitTx = function (tx) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, FranklinProvider.axiosRequest(axios_1.default.post(this.providerAddress + '/api/v0.1/submit_tx', tx))];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getTokens = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, FranklinProvider.axiosRequest(axios_1.default.get(this.providerAddress + '/api/v0.1/tokens'))];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getTransactionsHistory = function (address, offset, limit) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, FranklinProvider.axiosRequest(axios_1.default.get(this.providerAddress + "/api/v0.1/account/0x" + address.toString("hex") + "/history/" + offset + "/" + limit))];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getState = function (address) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, FranklinProvider.axiosRequest(axios_1.default.get(this.providerAddress + '/api/v0.1/account/' + ("0x" + address.toString("hex"))))];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getTxReceipt = function (tx_hash) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, FranklinProvider.axiosRequest(axios_1.default.get(this.providerAddress + '/api/v0.1/transactions/' + tx_hash))];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getPriorityOpReceipt = function (pq_id) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, FranklinProvider.axiosRequest(axios_1.default.get(this.providerAddress + "/api/v0.1/priority_operations/" + pq_id + "/"))];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    return FranklinProvider;
}());
exports.FranklinProvider = FranklinProvider;
var WalletKeys = /** @class */ (function () {
    function WalletKeys(privateKey) {
        this.privateKey = privateKey;
        this.publicKey = crypto_1.privateKeyToPublicKey(privateKey);
    }
    WalletKeys.prototype.signTransfer = function (tx) {
        var type = Buffer.from([5]); // tx type
        var from = tx.from;
        var to = tx.to;
        var token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token, 0);
        var bnAmount = new BN(bigNumberify(tx.amount).toString());
        var amount = utils_1.packAmount(bnAmount);
        var bnFee = new BN(bigNumberify(tx.fee).toString());
        var fee = utils_1.packFee(bnFee);
        var nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);
        var msg = Buffer.concat([type, from, to, token, amount, fee, nonce]);
        return crypto_1.signTransactionBytes(this.privateKey, msg);
    };
    WalletKeys.prototype.signWithdraw = function (tx) {
        var type = Buffer.from([3]);
        var account = tx.account;
        var eth_address = Buffer.from(tx.eth_address.slice(2), "hex");
        var token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token, 0);
        var bnAmount = new BN(bigNumberify(tx.amount).toString());
        var amount = bnAmount.toArrayLike(Buffer, "be", 16);
        var bnFee = new BN(bigNumberify(tx.fee).toString());
        var fee = utils_1.packFee(bnFee);
        var nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);
        var msg = Buffer.concat([type, account, eth_address, token, amount, fee, nonce]);
        return crypto_1.signTransactionBytes(this.privateKey, msg);
    };
    WalletKeys.prototype.signClose = function (tx) {
        var type = Buffer.from([4]);
        var account = tx.account;
        var nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);
        var msg = Buffer.concat([type, account, nonce]);
        return crypto_1.signTransactionBytes(this.privateKey, msg);
    };
    WalletKeys.prototype.signFullExit = function (op) {
        var type = Buffer.from([6]);
        var packed_pubkey = crypto_1.serializePointPacked(this.publicKey);
        var eth_address = Buffer.from(op.eth_address.slice(2), "hex");
        var token = Buffer.alloc(2);
        token.writeUInt16BE(op.token, 0);
        var nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(op.nonce, 0);
        var msg = Buffer.concat([type, packed_pubkey, eth_address, token, nonce]);
        return Buffer.from(crypto_1.signTransactionBytes(this.privateKey, msg).sign, "hex");
    };
    return WalletKeys;
}());
exports.WalletKeys = WalletKeys;
var Wallet = /** @class */ (function () {
    function Wallet(seed, provider, ethWallet, ethAddress) {
        this.provider = provider;
        this.ethWallet = ethWallet;
        this.ethAddress = ethAddress;
        var privateKey = crypto_1.privateKeyFromSeed(seed).privateKey;
        this.walletKeys = new WalletKeys(privateKey);
        this.address = crypto_1.pubkeyToAddress(this.walletKeys.publicKey);
    }
    Wallet.prototype.depositETH = function (amount) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract, tx;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
                        return [4 /*yield*/, franklinDeployedContract.depositETH(this.address, { value: amount, gasLimit: bigNumberify("200000") })];
                    case 1:
                        tx = _a.sent();
                        return [2 /*return*/, tx.hash];
                }
            });
        });
    };
    Wallet.prototype.approveERC20 = function (token, amount) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract, erc20DeployedToken;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
                        erc20DeployedToken = new ethers_1.Contract(token.address, IERC20Conract.abi, this.ethWallet);
                        return [4 /*yield*/, erc20DeployedToken.approve(franklinDeployedContract.address, amount)];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Wallet.prototype.depositApprovedERC20 = function (token, amount) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract, erc20DeployedToken, tx;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
                        erc20DeployedToken = new ethers_1.Contract(token.address, IERC20Conract.abi, this.ethWallet);
                        return [4 /*yield*/, franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, this.address, { gasLimit: bigNumberify("300000"), value: parseEther("0.05") })];
                    case 1:
                        tx = _a.sent();
                        return [2 /*return*/, tx.hash];
                }
            });
        });
    };
    Wallet.prototype.deposit = function (token, amount) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!(token.id == 0)) return [3 /*break*/, 2];
                        return [4 /*yield*/, this.depositETH(amount)];
                    case 1: return [2 /*return*/, _a.sent()];
                    case 2: return [4 /*yield*/, this.approveERC20(token, amount)];
                    case 3:
                        _a.sent();
                        return [4 /*yield*/, this.depositApprovedERC20(token, amount)];
                    case 4: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    // TODO: remove this method
    Wallet.prototype.waitTxReceipt = function (tx_hash) {
        return __awaiter(this, void 0, void 0, function () {
            var receipt;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!true) return [3 /*break*/, 3];
                        return [4 /*yield*/, this.provider.getTxReceipt(tx_hash)];
                    case 1:
                        receipt = _a.sent();
                        if (receipt != null) {
                            return [2 /*return*/, receipt];
                        }
                        return [4 /*yield*/, sleep(1000)];
                    case 2:
                        _a.sent();
                        return [3 /*break*/, 0];
                    case 3: return [2 /*return*/];
                }
            });
        });
    };
    Wallet.prototype.widthdrawOnchain = function (token, amount) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
                        if (!(token.id == 0)) return [3 /*break*/, 2];
                        return [4 /*yield*/, franklinDeployedContract.withdrawETH(amount, { gasLimit: 200000 })];
                    case 1: return [2 /*return*/, _a.sent()];
                    case 2: return [4 /*yield*/, franklinDeployedContract.withdrawERC20(token.address, amount, { gasLimit: bigNumberify("150000") })];
                    case 3: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Wallet.prototype.widthdrawOffchain = function (token, amount, fee) {
        return __awaiter(this, void 0, void 0, function () {
            var tx, _a, signature, tx_req;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        _a = {
                            account: this.address
                        };
                        return [4 /*yield*/, this.ethWallet.getAddress()];
                    case 1:
                        _a.eth_address = _b.sent(),
                            _a.token = token.id,
                            _a.amount = amount,
                            _a.fee = fee;
                        return [4 /*yield*/, this.getNonce()];
                    case 2:
                        tx = (_a.nonce = _b.sent(),
                            _a);
                        signature = this.walletKeys.signWithdraw(tx);
                        tx_req = FranklinProvider.prepareWithdrawRequestForNode(tx, signature);
                        return [4 /*yield*/, this.provider.submitTx(tx_req)];
                    case 3: return [2 /*return*/, _b.sent()];
                }
            });
        });
    };
    Wallet.prototype.emergencyWithdraw = function (token) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract, nonce, signature, _a, _b, _c, tx;
            return __generator(this, function (_d) {
                switch (_d.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
                        return [4 /*yield*/, this.getNonce()];
                    case 1:
                        nonce = _d.sent();
                        _b = (_a = this.walletKeys).signFullExit;
                        _c = { token: token.id };
                        return [4 /*yield*/, this.ethWallet.getAddress()];
                    case 2:
                        signature = _b.apply(_a, [(_c.eth_address = _d.sent(), _c.nonce = nonce, _c)]);
                        return [4 /*yield*/, franklinDeployedContract.fullExit(crypto_1.serializePointPacked(this.walletKeys.publicKey), token.address, signature, nonce, { gasLimit: bigNumberify("500000"), value: parseEther("0.02") })];
                    case 3:
                        tx = _d.sent();
                        return [2 /*return*/, tx.hash];
                }
            });
        });
    };
    Wallet.prototype.transfer = function (to, token, amount, fee) {
        return __awaiter(this, void 0, void 0, function () {
            var tx, _a, signature, tx_req;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        _a = {
                            from: this.address,
                            to: toAddress(to),
                            token: token.id,
                            amount: amount,
                            fee: fee
                        };
                        return [4 /*yield*/, this.getNonce()];
                    case 1:
                        tx = (_a.nonce = _b.sent(),
                            _a);
                        signature = this.walletKeys.signTransfer(tx);
                        tx_req = FranklinProvider.prepareTransferRequestForNode(tx, signature);
                        return [4 /*yield*/, this.provider.submitTx(tx_req)];
                    case 2: return [2 /*return*/, _b.sent()];
                }
            });
        });
    };
    Wallet.prototype.close = function () {
        return __awaiter(this, void 0, void 0, function () {
            var tx, _a, signature, tx_req;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        _a = {
                            account: this.address
                        };
                        return [4 /*yield*/, this.getNonce()];
                    case 1:
                        tx = (_a.nonce = _b.sent(),
                            _a);
                        signature = this.walletKeys.signClose(tx);
                        tx_req = FranklinProvider.prepareCloseRequestForNode(tx, signature);
                        return [4 /*yield*/, this.provider.submitTx(tx_req)];
                    case 2: return [2 /*return*/, _b.sent()];
                }
            });
        });
    };
    Wallet.prototype.getNonce = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!(this.pendingNonce == null)) return [3 /*break*/, 2];
                        return [4 /*yield*/, this.fetchFranklinState()];
                    case 1:
                        _a.sent();
                        this.pendingNonce = this.franklinState.commited.nonce + this.franklinState.pending_txs.length;
                        _a.label = 2;
                    case 2: return [2 /*return*/, this.pendingNonce++];
                }
            });
        });
    };
    Wallet.fromEthWallet = function (wallet, franklinProvider) {
        if (franklinProvider === void 0) { franklinProvider = new FranklinProvider(); }
        return __awaiter(this, void 0, void 0, function () {
            var seed, ethAddress, frankinWallet;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, wallet.signMessage('Matter login')];
                    case 1:
                        seed = (_a.sent()).substr(2);
                        return [4 /*yield*/, wallet.getAddress()];
                    case 2:
                        ethAddress = _a.sent();
                        frankinWallet = new Wallet(Buffer.from(seed, 'hex'), franklinProvider, wallet, ethAddress);
                        return [2 /*return*/, frankinWallet];
                }
            });
        });
    };
    Wallet.prototype.fetchEthState = function () {
        return __awaiter(this, void 0, void 0, function () {
            var onchainBalances, contractBalances, franklinDeployedContract, _i, _a, token, _b, _c, erc20DeployedToken, _d, _e, _f, _g;
            return __generator(this, function (_h) {
                switch (_h.label) {
                    case 0:
                        onchainBalances = new Array(this.supportedTokens.length);
                        contractBalances = new Array(this.supportedTokens.length);
                        franklinDeployedContract = new ethers_1.Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
                        _i = 0, _a = this.supportedTokens;
                        _h.label = 1;
                    case 1:
                        if (!(_i < _a.length)) return [3 /*break*/, 8];
                        token = _a[_i];
                        if (!(token.id == 0)) return [3 /*break*/, 3];
                        _b = onchainBalances;
                        _c = token.id;
                        return [4 /*yield*/, this.ethWallet.provider.getBalance(this.ethAddress)];
                    case 2:
                        _b[_c] = _h.sent();
                        return [3 /*break*/, 5];
                    case 3:
                        erc20DeployedToken = new ethers_1.Contract(token.address, IERC20Conract.abi, this.ethWallet);
                        _d = onchainBalances;
                        _e = token.id;
                        return [4 /*yield*/, erc20DeployedToken.balanceOf(this.ethAddress).then(function (n) { return n.toString(); })];
                    case 4:
                        _d[_e] = _h.sent();
                        _h.label = 5;
                    case 5:
                        _f = contractBalances;
                        _g = token.id;
                        return [4 /*yield*/, franklinDeployedContract.balancesToWithdraw(this.ethAddress, token.id)];
                    case 6:
                        _f[_g] = _h.sent();
                        _h.label = 7;
                    case 7:
                        _i++;
                        return [3 /*break*/, 1];
                    case 8:
                        this.ethState = { onchainBalances: onchainBalances, contractBalances: contractBalances };
                        return [2 /*return*/];
                }
            });
        });
    };
    Wallet.prototype.fetchFranklinState = function () {
        return __awaiter(this, void 0, void 0, function () {
            var _a, _b;
            return __generator(this, function (_c) {
                switch (_c.label) {
                    case 0:
                        _a = this;
                        return [4 /*yield*/, this.provider.getTokens()];
                    case 1:
                        _a.supportedTokens = _c.sent();
                        _b = this;
                        return [4 /*yield*/, this.provider.getState(this.address)];
                    case 2:
                        _b.franklinState = _c.sent();
                        return [2 /*return*/];
                }
            });
        });
    };
    Wallet.prototype.updateState = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.fetchFranklinState()];
                    case 1:
                        _a.sent();
                        return [4 /*yield*/, this.fetchEthState()];
                    case 2:
                        _a.sent();
                        return [2 /*return*/];
                }
            });
        });
    };
    Wallet.prototype.waitPendingTxsExecuted = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.fetchFranklinState()];
                    case 1:
                        _a.sent();
                        _a.label = 2;
                    case 2:
                        if (!(this.franklinState.pending_txs.length > 0)) return [3 /*break*/, 4];
                        return [4 /*yield*/, this.fetchFranklinState()];
                    case 3:
                        _a.sent();
                        return [3 /*break*/, 2];
                    case 4: return [2 /*return*/];
                }
            });
        });
    };
    return Wallet;
}());
exports.Wallet = Wallet;
