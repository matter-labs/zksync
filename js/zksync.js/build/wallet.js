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
Object.defineProperty(exports, "__esModule", { value: true });
var ethers_1 = require("ethers");
var signer_1 = require("./signer");
var utils_1 = require("./utils");
var crypto_1 = require("./crypto");
var Wallet = /** @class */ (function () {
    function Wallet(signer) {
        this.signer = signer;
    }
    Wallet.prototype.connect = function (provider, ethProxy) {
        this.provider = provider;
        this.ethProxy = ethProxy;
        return this;
    };
    Wallet.prototype.syncTransfer = function (transfer) {
        return __awaiter(this, void 0, void 0, function () {
            var tokenId, nonce, _a, transactionData, signedTransferTransaction, transactionHash;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0: return [4 /*yield*/, this.ethProxy.resolveTokenId(transfer.token)];
                    case 1:
                        tokenId = _b.sent();
                        if (!(transfer.nonce != null)) return [3 /*break*/, 3];
                        return [4 /*yield*/, this.getNonce(transfer.nonce)];
                    case 2:
                        _a = _b.sent();
                        return [3 /*break*/, 5];
                    case 3: return [4 /*yield*/, this.getNonce()];
                    case 4:
                        _a = _b.sent();
                        _b.label = 5;
                    case 5:
                        nonce = _a;
                        transactionData = {
                            to: transfer.to,
                            tokenId: tokenId,
                            amount: transfer.amount,
                            fee: transfer.fee,
                            nonce: nonce
                        };
                        signedTransferTransaction = this.signer.signSyncTransfer(transactionData);
                        return [4 /*yield*/, this.provider.submitTx(signedTransferTransaction)];
                    case 6:
                        transactionHash = _b.sent();
                        return [2 /*return*/, new Transaction(signedTransferTransaction, transactionHash, this.provider)];
                }
            });
        });
    };
    Wallet.prototype.withdrawTo = function (withdraw) {
        return __awaiter(this, void 0, void 0, function () {
            var tokenId, nonce, _a, transactionData, signedWithdrawTransaction, submitResponse;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0: return [4 /*yield*/, this.ethProxy.resolveTokenId(withdraw.token)];
                    case 1:
                        tokenId = _b.sent();
                        if (!(withdraw.nonce != null)) return [3 /*break*/, 3];
                        return [4 /*yield*/, this.getNonce(withdraw.nonce)];
                    case 2:
                        _a = _b.sent();
                        return [3 /*break*/, 5];
                    case 3: return [4 /*yield*/, this.getNonce()];
                    case 4:
                        _a = _b.sent();
                        _b.label = 5;
                    case 5:
                        nonce = _a;
                        transactionData = {
                            ethAddress: withdraw.ethAddress,
                            tokenId: tokenId,
                            amount: withdraw.amount,
                            fee: withdraw.fee,
                            nonce: nonce
                        };
                        signedWithdrawTransaction = this.signer.signSyncWithdraw(transactionData);
                        return [4 /*yield*/, this.provider.submitTx(signedWithdrawTransaction)];
                    case 6:
                        submitResponse = _b.sent();
                        return [2 /*return*/, new Transaction(signedWithdrawTransaction, submitResponse, this.provider)];
                }
            });
        });
    };
    Wallet.prototype.close = function (nonce) {
        if (nonce === void 0) { nonce = "committed"; }
        return __awaiter(this, void 0, void 0, function () {
            var numNonce, signedCloseTransaction, transactionHash;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getNonce(nonce)];
                    case 1:
                        numNonce = _a.sent();
                        signedCloseTransaction = this.signer.signSyncCloseAccount({
                            nonce: numNonce
                        });
                        return [4 /*yield*/, this.provider.submitTx(signedCloseTransaction)];
                    case 2:
                        transactionHash = _a.sent();
                        return [2 /*return*/, new Transaction(signedCloseTransaction, transactionHash, this.provider)];
                }
            });
        });
    };
    Wallet.prototype.getNonce = function (nonce) {
        if (nonce === void 0) { nonce = "committed"; }
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!(nonce == "committed")) return [3 /*break*/, 2];
                        return [4 /*yield*/, this.provider.getState(this.signer.address())];
                    case 1: return [2 /*return*/, (_a.sent())
                            .committed.nonce];
                    case 2:
                        if (typeof nonce == "number") {
                            return [2 /*return*/, nonce];
                        }
                        _a.label = 3;
                    case 3: return [2 /*return*/];
                }
            });
        });
    };
    Wallet.prototype.address = function () {
        return this.signer.address();
    };
    Wallet.fromEthSigner = function (ethWallet, provider, ethProxy) {
        return __awaiter(this, void 0, void 0, function () {
            var seedHex, seed, signer, wallet;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, ethWallet.signMessage("Matter login")];
                    case 1:
                        seedHex = (_a.sent()).substr(2);
                        seed = Buffer.from(seedHex, "hex");
                        signer = signer_1.Signer.fromSeed(seed);
                        wallet = new Wallet(signer);
                        if (provider && ethProxy) {
                            wallet.connect(provider, ethProxy);
                        }
                        return [2 /*return*/, wallet];
                }
            });
        });
    };
    Wallet.prototype.getAccountState = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                return [2 /*return*/, this.provider.getState(this.signer.address())];
            });
        });
    };
    Wallet.prototype.getBalance = function (token, type) {
        if (type === void 0) { type = "committed"; }
        return __awaiter(this, void 0, void 0, function () {
            var accountState, balance;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getAccountState()];
                    case 1:
                        accountState = _a.sent();
                        if (token != "ETH") {
                            token = token.toLowerCase();
                        }
                        if (type == "committed") {
                            balance = accountState.committed.balances[token] || "0";
                        }
                        else {
                            balance = accountState.verified.balances[token] || "0";
                        }
                        return [2 /*return*/, ethers_1.utils.bigNumberify(balance)];
                }
            });
        });
    };
    return Wallet;
}());
exports.Wallet = Wallet;
function depositFromETH(deposit) {
    return __awaiter(this, void 0, void 0, function () {
        var gasPrice, maxFeeInETHToken, baseFee, mainZkSyncContract, ethTransaction, erc20contract, approveTx;
        return __generator(this, function (_a) {
            switch (_a.label) {
                case 0: return [4 /*yield*/, deposit.depositFrom.provider.getGasPrice()];
                case 1:
                    gasPrice = _a.sent();
                    if (!(deposit.maxFeeInETHToken != null)) return [3 /*break*/, 2];
                    maxFeeInETHToken = deposit.maxFeeInETHToken;
                    return [3 /*break*/, 4];
                case 2: return [4 /*yield*/, deposit.depositTo.ethProxy.estimateDepositFeeInETHToken(deposit.token, gasPrice)];
                case 3:
                    baseFee = _a.sent();
                    maxFeeInETHToken = baseFee;
                    _a.label = 4;
                case 4:
                    mainZkSyncContract = new ethers_1.Contract(deposit.depositTo.provider.contractAddress.mainContract, utils_1.SYNC_MAIN_CONTRACT_INTERFACE, deposit.depositFrom);
                    if (!(deposit.token == "ETH")) return [3 /*break*/, 6];
                    return [4 /*yield*/, mainZkSyncContract.depositETH(deposit.amount, deposit.depositTo.address().replace("sync:", "0x"), {
                            value: ethers_1.utils.bigNumberify(deposit.amount).add(maxFeeInETHToken),
                            gasLimit: ethers_1.utils.bigNumberify("200000"),
                            gasPrice: gasPrice
                        })];
                case 5:
                    ethTransaction = _a.sent();
                    return [3 /*break*/, 9];
                case 6:
                    erc20contract = new ethers_1.Contract(deposit.token, utils_1.IERC20_INTERFACE, deposit.depositFrom);
                    return [4 /*yield*/, erc20contract.approve(deposit.depositTo.provider.contractAddress.mainContract, deposit.amount)];
                case 7:
                    approveTx = _a.sent();
                    return [4 /*yield*/, mainZkSyncContract.depositERC20(deposit.token, deposit.amount, deposit.depositTo.address().replace("sync:", "0x"), {
                            gasLimit: ethers_1.utils.bigNumberify("250000"),
                            value: maxFeeInETHToken,
                            nonce: approveTx.nonce + 1,
                            gasPrice: gasPrice
                        })];
                case 8:
                    ethTransaction = _a.sent();
                    _a.label = 9;
                case 9: return [2 /*return*/, new ETHOperation(ethTransaction, deposit.depositTo.provider)];
            }
        });
    });
}
exports.depositFromETH = depositFromETH;
function emergencyWithdraw(withdraw) {
    return __awaiter(this, void 0, void 0, function () {
        var gasPrice, maxFeeInETHToken, baseFee, accountId, accountState, tokenId, nonce, _a, emergencyWithdrawSignature, _b, _c, _d, mainZkSyncContract, tokenAddress, ethTransaction;
        return __generator(this, function (_e) {
            switch (_e.label) {
                case 0: return [4 /*yield*/, withdraw.withdrawTo.provider.getGasPrice()];
                case 1:
                    gasPrice = _e.sent();
                    if (!(withdraw.maxFeeInETHToken != null)) return [3 /*break*/, 2];
                    maxFeeInETHToken = withdraw.maxFeeInETHToken;
                    return [3 /*break*/, 4];
                case 2: return [4 /*yield*/, withdraw.withdrawFrom.ethProxy.estimateEmergencyWithdrawFeeInETHToken(gasPrice)];
                case 3:
                    baseFee = _e.sent();
                    maxFeeInETHToken = baseFee;
                    _e.label = 4;
                case 4:
                    if (!(withdraw.accountId != null)) return [3 /*break*/, 5];
                    accountId = withdraw.accountId;
                    return [3 /*break*/, 7];
                case 5: return [4 /*yield*/, withdraw.withdrawFrom.getAccountState()];
                case 6:
                    accountState = _e.sent();
                    if (!accountState.id) {
                        throw new Error("Can't resolve account id from the ZK Sync node");
                    }
                    accountId = accountState.id;
                    _e.label = 7;
                case 7: return [4 /*yield*/, withdraw.withdrawFrom.ethProxy.resolveTokenId(withdraw.token)];
                case 8:
                    tokenId = _e.sent();
                    if (!(withdraw.nonce != null)) return [3 /*break*/, 10];
                    return [4 /*yield*/, withdraw.withdrawFrom.getNonce(withdraw.nonce)];
                case 9:
                    _a = _e.sent();
                    return [3 /*break*/, 12];
                case 10: return [4 /*yield*/, withdraw.withdrawFrom.getNonce()];
                case 11:
                    _a = _e.sent();
                    _e.label = 12;
                case 12:
                    nonce = _a;
                    _c = (_b = withdraw.withdrawFrom.signer).syncEmergencyWithdrawSignature;
                    _d = {
                        accountId: accountId
                    };
                    return [4 /*yield*/, withdraw.withdrawTo.getAddress()];
                case 13:
                    emergencyWithdrawSignature = _c.apply(_b, [(_d.ethAddress = _e.sent(),
                            _d.tokenId = tokenId,
                            _d.nonce = nonce,
                            _d)]);
                    mainZkSyncContract = new ethers_1.Contract(withdraw.withdrawFrom.ethProxy.contractAddress.mainContract, utils_1.SYNC_MAIN_CONTRACT_INTERFACE, withdraw.withdrawTo);
                    tokenAddress = "0x0000000000000000000000000000000000000000";
                    if (withdraw.token != "ETH") {
                        tokenAddress = withdraw.token;
                    }
                    return [4 /*yield*/, mainZkSyncContract.fullExit(accountId, crypto_1.serializePointPacked(withdraw.withdrawFrom.signer.publicKey), tokenAddress, emergencyWithdrawSignature, nonce, {
                            gasLimit: ethers_1.utils.bigNumberify("500000"),
                            value: maxFeeInETHToken,
                            gasPrice: gasPrice
                        })];
                case 14:
                    ethTransaction = _e.sent();
                    return [2 /*return*/, new ETHOperation(ethTransaction, withdraw.withdrawFrom.provider)];
            }
        });
    });
}
exports.emergencyWithdraw = emergencyWithdraw;
function getEthereumBalance(ethSigner, token) {
    return __awaiter(this, void 0, void 0, function () {
        var balance, _a, _b, erc20contract, _c, _d;
        return __generator(this, function (_e) {
            switch (_e.label) {
                case 0:
                    if (!(token == "ETH")) return [3 /*break*/, 3];
                    _b = (_a = ethSigner.provider).getBalance;
                    return [4 /*yield*/, ethSigner.getAddress()];
                case 1: return [4 /*yield*/, _b.apply(_a, [_e.sent()])];
                case 2:
                    balance = _e.sent();
                    return [3 /*break*/, 6];
                case 3:
                    erc20contract = new ethers_1.Contract(token, utils_1.IERC20_INTERFACE, ethSigner);
                    _d = (_c = erc20contract).balanceOf;
                    return [4 /*yield*/, ethSigner.getAddress()];
                case 4: return [4 /*yield*/, _d.apply(_c, [_e.sent()])];
                case 5:
                    balance = _e.sent();
                    _e.label = 6;
                case 6: return [2 /*return*/, balance];
            }
        });
    });
}
exports.getEthereumBalance = getEthereumBalance;
var ETHOperation = /** @class */ (function () {
    function ETHOperation(ethTx, zkSyncProvider) {
        this.ethTx = ethTx;
        this.zkSyncProvider = zkSyncProvider;
        this.state = "Sent";
    }
    ETHOperation.prototype.awaitEthereumTxCommit = function () {
        return __awaiter(this, void 0, void 0, function () {
            var txReceipt, _i, _a, log, priorityQueueLog;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        if (this.state != "Sent")
                            return [2 /*return*/];
                        return [4 /*yield*/, this.ethTx.wait()];
                    case 1:
                        txReceipt = _b.sent();
                        for (_i = 0, _a = txReceipt.logs; _i < _a.length; _i++) {
                            log = _a[_i];
                            priorityQueueLog = utils_1.SYNC_PRIOR_QUEUE_INTERFACE.parseLog(log);
                            if (priorityQueueLog) {
                                this.priorityOpId = priorityQueueLog.values.serialId;
                            }
                        }
                        if (!this.priorityOpId) {
                            throw new Error("Failed to parse tx logs");
                        }
                        this.state = "Mined";
                        return [2 /*return*/, txReceipt];
                }
            });
        });
    };
    ETHOperation.prototype.awaitReceipt = function () {
        return __awaiter(this, void 0, void 0, function () {
            var receipt;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.awaitEthereumTxCommit()];
                    case 1:
                        _a.sent();
                        if (this.state != "Mined")
                            return [2 /*return*/];
                        return [4 /*yield*/, this.zkSyncProvider.notifyPriorityOp(this.priorityOpId.toNumber(), "COMMIT")];
                    case 2:
                        receipt = _a.sent();
                        this.state = "Committed";
                        return [2 /*return*/, receipt];
                }
            });
        });
    };
    ETHOperation.prototype.awaitVerifyReceipt = function () {
        return __awaiter(this, void 0, void 0, function () {
            var receipt;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.awaitReceipt()];
                    case 1:
                        _a.sent();
                        if (this.state != "Committed")
                            return [2 /*return*/];
                        return [4 /*yield*/, this.zkSyncProvider.notifyPriorityOp(this.priorityOpId.toNumber(), "VERIFY")];
                    case 2:
                        receipt = _a.sent();
                        this.state = "Verified";
                        return [2 /*return*/, receipt];
                }
            });
        });
    };
    return ETHOperation;
}());
var Transaction = /** @class */ (function () {
    function Transaction(txData, txHash, sidechainProvider) {
        this.txData = txData;
        this.txHash = txHash;
        this.sidechainProvider = sidechainProvider;
        this.state = "Sent";
    }
    Transaction.prototype.awaitReceipt = function () {
        return __awaiter(this, void 0, void 0, function () {
            var receipt;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (this.state !== "Sent")
                            return [2 /*return*/];
                        return [4 /*yield*/, this.sidechainProvider.notifyTransaction(this.txHash, "COMMIT")];
                    case 1:
                        receipt = _a.sent();
                        this.state = "Committed";
                        return [2 /*return*/, receipt];
                }
            });
        });
    };
    Transaction.prototype.awaitVerifyReceipt = function () {
        return __awaiter(this, void 0, void 0, function () {
            var receipt;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.awaitReceipt()];
                    case 1:
                        _a.sent();
                        return [4 /*yield*/, this.sidechainProvider.notifyTransaction(this.txHash, "VERIFY")];
                    case 2:
                        receipt = _a.sent();
                        this.state = "Verified";
                        return [2 /*return*/, receipt];
                }
            });
        });
    };
    return Transaction;
}());
