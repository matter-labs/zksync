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
var provider_1 = require("./provider");
var signer_1 = require("./signer");
var utils_1 = require("./utils");
var Wallet = /** @class */ (function () {
    function Wallet(signer, ethSigner, cachedAddress, tokensCache) {
        this.signer = signer;
        this.ethSigner = ethSigner;
        this.cachedAddress = cachedAddress;
        this.tokensCache = tokensCache;
    }
    Wallet.prototype.connect = function (provider) {
        this.provider = provider;
        return this;
    };
    Wallet.prototype.syncTransfer = function (transfer) {
        return __awaiter(this, void 0, void 0, function () {
            var tokenId, nonce, _a, transactionData, signedTransferTransaction, transactionHash;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0: return [4 /*yield*/, this.tokensCache.resolveTokenId(transfer.token)];
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
                            from: this.address(),
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
            var withdrawAddress, tokenId, nonce, _a, transactionData, signedWithdrawTransaction, submitResponse;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        withdrawAddress = withdraw.ethAddress == null ? this.address() : withdraw.ethAddress;
                        return [4 /*yield*/, this.tokensCache.resolveTokenId(withdraw.token)];
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
                            from: this.address(),
                            ethAddress: withdrawAddress,
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
    Wallet.prototype.isCurrentPubkeySet = function () {
        return __awaiter(this, void 0, void 0, function () {
            var currentPubKeyHash, signerPubKeyHash;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getCurrentPubKeyHash()];
                    case 1:
                        currentPubKeyHash = _a.sent();
                        signerPubKeyHash = this.signer.pubKeyHash();
                        return [2 /*return*/, currentPubKeyHash === signerPubKeyHash];
                }
            });
        });
    };
    Wallet.prototype.setCurrentPubkeyWithZksyncTx = function (nonce, onchainAuth) {
        if (nonce === void 0) { nonce = "committed"; }
        if (onchainAuth === void 0) { onchainAuth = false; }
        return __awaiter(this, void 0, void 0, function () {
            var currentPubKeyHash, newPubKeyHash, numNonce, newPkHash, message, ethSignature, _a, txData, transactionHash;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0: return [4 /*yield*/, this.getCurrentPubKeyHash()];
                    case 1:
                        currentPubKeyHash = _b.sent();
                        newPubKeyHash = this.signer.pubKeyHash();
                        if (currentPubKeyHash == newPubKeyHash) {
                            throw new Error("Current PubKeyHash is the same as new");
                        }
                        return [4 /*yield*/, this.getNonce(nonce)];
                    case 2:
                        numNonce = _b.sent();
                        newPkHash = signer_1.serializeAddress(newPubKeyHash);
                        message = Buffer.concat([signer_1.serializeNonce(numNonce), newPkHash]);
                        if (!onchainAuth) return [3 /*break*/, 3];
                        _a = null;
                        return [3 /*break*/, 5];
                    case 3: return [4 /*yield*/, this.ethSigner.signMessage(message)];
                    case 4:
                        _a = _b.sent();
                        _b.label = 5;
                    case 5:
                        ethSignature = _a;
                        txData = {
                            type: "ChangePubKey",
                            account: this.address(),
                            newPkHash: this.signer.pubKeyHash(),
                            nonce: numNonce,
                            ethSignature: ethSignature
                        };
                        return [4 /*yield*/, this.provider.submitTx(txData)];
                    case 6:
                        transactionHash = _b.sent();
                        return [2 /*return*/, new Transaction(txData, transactionHash, this.provider)];
                }
            });
        });
    };
    Wallet.prototype.authChangePubkey = function (nonce) {
        if (nonce === void 0) { nonce = "committed"; }
        return __awaiter(this, void 0, void 0, function () {
            var currentPubKeyHash, newPubKeyHash, numNonce, mainZkSyncContract, ethTransaction;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getCurrentPubKeyHash()];
                    case 1:
                        currentPubKeyHash = _a.sent();
                        newPubKeyHash = this.signer.pubKeyHash();
                        if (currentPubKeyHash == newPubKeyHash) {
                            throw new Error("Current PubKeyHash is the same as new");
                        }
                        return [4 /*yield*/, this.getNonce(nonce)];
                    case 2:
                        numNonce = _a.sent();
                        mainZkSyncContract = new ethers_1.Contract(this.provider.contractAddress.mainContract, utils_1.SYNC_MAIN_CONTRACT_INTERFACE, this.ethSigner);
                        return [4 /*yield*/, mainZkSyncContract.authPubkeyHash(newPubKeyHash.replace("sync:", "0x"), numNonce, {
                                gasLimit: ethers_1.utils.bigNumberify("200000")
                            })];
                    case 3:
                        ethTransaction = _a.sent();
                        return [2 /*return*/, ethTransaction];
                }
            });
        });
    };
    Wallet.prototype.getCurrentPubKeyHash = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.provider.getState(this.address())];
                    case 1: return [2 /*return*/, (_a.sent()).committed
                            .pubKeyHash];
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
                        return [4 /*yield*/, this.provider.getState(this.address())];
                    case 1: return [2 /*return*/, (_a.sent()).committed
                            .nonce];
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
        return this.cachedAddress;
    };
    Wallet.fromEthSigner = function (ethWallet, provider) {
        return __awaiter(this, void 0, void 0, function () {
            var seedHex, seed, signer, tokenCache, _a, wallet, _b, _c;
            return __generator(this, function (_d) {
                switch (_d.label) {
                    case 0: return [4 /*yield*/, ethWallet.signMessage("Matter login")];
                    case 1:
                        seedHex = (_d.sent()).substr(2);
                        seed = Buffer.from(seedHex, "hex");
                        signer = signer_1.Signer.fromSeed(seed);
                        _a = utils_1.TokenSet.bind;
                        return [4 /*yield*/, provider.getTokens()];
                    case 2:
                        tokenCache = new (_a.apply(utils_1.TokenSet, [void 0, _d.sent()]))();
                        _b = Wallet.bind;
                        _c = [void 0, signer,
                            ethWallet];
                        return [4 /*yield*/, ethWallet.getAddress()];
                    case 3:
                        wallet = new (_b.apply(Wallet, _c.concat([_d.sent(),
                            tokenCache])))();
                        wallet.connect(provider);
                        return [2 /*return*/, wallet];
                }
            });
        });
    };
    Wallet.prototype.getAccountState = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                return [2 /*return*/, this.provider.getState(this.address())];
            });
        });
    };
    Wallet.prototype.getBalance = function (token, type) {
        if (type === void 0) { type = "committed"; }
        return __awaiter(this, void 0, void 0, function () {
            var accountState, tokenSymbol, balance;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getAccountState()];
                    case 1:
                        accountState = _a.sent();
                        tokenSymbol = this.tokensCache.resolveTokenSymbol(token);
                        if (type === "committed") {
                            balance = accountState.committed.balances[tokenSymbol] || "0";
                        }
                        else {
                            balance = accountState.verified.balances[tokenSymbol] || "0";
                        }
                        return [2 /*return*/, ethers_1.utils.bigNumberify(balance)];
                }
            });
        });
    };
    Wallet.prototype.getEthereumBalance = function (token) {
        return __awaiter(this, void 0, void 0, function () {
            var balance, erc20contract;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!utils_1.isTokenETH(token)) return [3 /*break*/, 2];
                        return [4 /*yield*/, this.ethSigner.provider.getBalance(this.cachedAddress)];
                    case 1:
                        balance = _a.sent();
                        return [3 /*break*/, 4];
                    case 2:
                        erc20contract = new ethers_1.Contract(this.tokensCache.resolveTokenAddress(token), utils_1.IERC20_INTERFACE, this.ethSigner);
                        return [4 /*yield*/, erc20contract.balanceOf(this.cachedAddress)];
                    case 3:
                        balance = _a.sent();
                        _a.label = 4;
                    case 4: return [2 /*return*/, balance];
                }
            });
        });
    };
    return Wallet;
}());
exports.Wallet = Wallet;
function depositFromETH(deposit) {
    return __awaiter(this, void 0, void 0, function () {
        var gasPrice, ethProxy, maxFeeInETHToken, baseFee, mainZkSyncContract, ethTransaction, tokenAddress, erc20contract, approveTx;
        return __generator(this, function (_a) {
            switch (_a.label) {
                case 0: return [4 /*yield*/, deposit.depositFrom.provider.getGasPrice()];
                case 1:
                    gasPrice = _a.sent();
                    ethProxy = new provider_1.ETHProxy(deposit.depositFrom.provider, deposit.depositTo.provider.contractAddress);
                    if (!(deposit.maxFeeInETHToken != null)) return [3 /*break*/, 2];
                    maxFeeInETHToken = deposit.maxFeeInETHToken;
                    return [3 /*break*/, 4];
                case 2: return [4 /*yield*/, ethProxy.estimateDepositFeeInETHToken(deposit.token, gasPrice)];
                case 3:
                    baseFee = _a.sent();
                    maxFeeInETHToken = baseFee;
                    _a.label = 4;
                case 4:
                    mainZkSyncContract = new ethers_1.Contract(deposit.depositTo.provider.contractAddress.mainContract, utils_1.SYNC_MAIN_CONTRACT_INTERFACE, deposit.depositFrom);
                    if (!utils_1.isTokenETH(deposit.token)) return [3 /*break*/, 6];
                    return [4 /*yield*/, mainZkSyncContract.depositETH(deposit.amount, deposit.depositTo.address(), {
                            value: ethers_1.utils.bigNumberify(deposit.amount).add(maxFeeInETHToken),
                            gasLimit: ethers_1.utils.bigNumberify("200000"),
                            gasPrice: gasPrice
                        })];
                case 5:
                    ethTransaction = _a.sent();
                    return [3 /*break*/, 9];
                case 6:
                    tokenAddress = deposit.depositTo.tokensCache.resolveTokenAddress(deposit.token);
                    erc20contract = new ethers_1.Contract(tokenAddress, utils_1.IERC20_INTERFACE, deposit.depositFrom);
                    return [4 /*yield*/, erc20contract.approve(deposit.depositTo.provider.contractAddress.mainContract, deposit.amount)];
                case 7:
                    approveTx = _a.sent();
                    return [4 /*yield*/, mainZkSyncContract.depositERC20(tokenAddress, deposit.amount, deposit.depositTo.address(), {
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
        var gasPrice, ethProxy, maxFeeInETHToken, accountId, accountState, mainZkSyncContract, tokenAddress, ethTransaction;
        return __generator(this, function (_a) {
            switch (_a.label) {
                case 0: return [4 /*yield*/, withdraw.withdrawFrom.ethSigner.provider.getGasPrice()];
                case 1:
                    gasPrice = _a.sent();
                    ethProxy = new provider_1.ETHProxy(withdraw.withdrawFrom.ethSigner.provider, withdraw.withdrawFrom.provider.contractAddress);
                    if (!(withdraw.maxFeeInETHToken != null)) return [3 /*break*/, 2];
                    maxFeeInETHToken = withdraw.maxFeeInETHToken;
                    return [3 /*break*/, 4];
                case 2: return [4 /*yield*/, ethProxy.estimateEmergencyWithdrawFeeInETHToken(gasPrice)];
                case 3:
                    maxFeeInETHToken = _a.sent();
                    _a.label = 4;
                case 4:
                    if (!(withdraw.accountId != null)) return [3 /*break*/, 5];
                    accountId = withdraw.accountId;
                    return [3 /*break*/, 7];
                case 5: return [4 /*yield*/, withdraw.withdrawFrom.getAccountState()];
                case 6:
                    accountState = _a.sent();
                    if (!accountState.id) {
                        throw new Error("Can't resolve account id from the ZK Sync node");
                    }
                    accountId = accountState.id;
                    _a.label = 7;
                case 7:
                    mainZkSyncContract = new ethers_1.Contract(ethProxy.contractAddress.mainContract, utils_1.SYNC_MAIN_CONTRACT_INTERFACE, withdraw.withdrawFrom.ethSigner);
                    tokenAddress = withdraw.withdrawFrom.tokensCache.resolveTokenAddress(withdraw.token);
                    return [4 /*yield*/, mainZkSyncContract.fullExit(accountId, tokenAddress, {
                            gasLimit: ethers_1.utils.bigNumberify("500000"),
                            value: maxFeeInETHToken,
                            gasPrice: gasPrice
                        })];
                case 8:
                    ethTransaction = _a.sent();
                    return [2 /*return*/, new ETHOperation(ethTransaction, withdraw.withdrawFrom.provider)];
            }
        });
    });
}
exports.emergencyWithdraw = emergencyWithdraw;
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
                            priorityQueueLog = utils_1.SYNC_MAIN_CONTRACT_INTERFACE.parseLog(log);
                            if (priorityQueueLog && priorityQueueLog.values.serialId != null) {
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
