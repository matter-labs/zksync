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
var transport_1 = require("./transport");
var ethers_1 = require("ethers");
var utils_1 = require("./utils");
function getDefaultProvider(network, transport) {
    if (transport === void 0) { transport = "WS"; }
    return __awaiter(this, void 0, void 0, function () {
        return __generator(this, function (_a) {
            switch (_a.label) {
                case 0:
                    if (!(network == "localhost")) return [3 /*break*/, 5];
                    if (!(transport == "WS")) return [3 /*break*/, 2];
                    return [4 /*yield*/, Provider.newWebsocketProvider("ws://127.0.0.1:3031")];
                case 1: return [2 /*return*/, _a.sent()];
                case 2:
                    if (!(transport == "HTTP")) return [3 /*break*/, 4];
                    return [4 /*yield*/, Provider.newHttpProvider("http://127.0.0.1:3030")];
                case 3: return [2 /*return*/, _a.sent()];
                case 4: return [3 /*break*/, 9];
                case 5:
                    if (!(network == "testnet")) return [3 /*break*/, 9];
                    if (!(transport == "WS")) return [3 /*break*/, 7];
                    return [4 /*yield*/, Provider.newWebsocketProvider("wss://testnet.matter-labs.io/jsrpc-ws")];
                case 6: return [2 /*return*/, _a.sent()];
                case 7:
                    if (!(transport == "HTTP")) return [3 /*break*/, 9];
                    return [4 /*yield*/, Provider.newHttpProvider("https://testnet.matter-labs.io/jsrpc")];
                case 8: return [2 /*return*/, _a.sent()];
                case 9: return [2 /*return*/];
            }
        });
    });
}
exports.getDefaultProvider = getDefaultProvider;
var Provider = /** @class */ (function () {
    function Provider(transport) {
        this.transport = transport;
    }
    Provider.newWebsocketProvider = function (address) {
        return __awaiter(this, void 0, void 0, function () {
            var transport, provider, _a;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0: return [4 /*yield*/, transport_1.WSTransport.connect(address)];
                    case 1:
                        transport = _b.sent();
                        provider = new Provider(transport);
                        _a = provider;
                        return [4 /*yield*/, provider.getContractAddress()];
                    case 2:
                        _a.contractAddress = _b.sent();
                        return [2 /*return*/, provider];
                }
            });
        });
    };
    Provider.newHttpProvider = function (address) {
        if (address === void 0) { address = "http://127.0.0.1:3030"; }
        return __awaiter(this, void 0, void 0, function () {
            var transport, provider, _a;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        transport = new transport_1.HTTPTransport(address);
                        provider = new Provider(transport);
                        _a = provider;
                        return [4 /*yield*/, provider.getContractAddress()];
                    case 1:
                        _a.contractAddress = _b.sent();
                        return [2 /*return*/, provider];
                }
            });
        });
    };
    // return transaction hash (e.g. sync-tx:dead..beef)
    Provider.prototype.submitTx = function (tx) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.request("tx_submit", [tx])];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Provider.prototype.getContractAddress = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.request("contract_address", null)];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Provider.prototype.getTokens = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.request("tokens", null)];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Provider.prototype.getState = function (address) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.request("account_info", [address])];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    // get transaction status by its hash (e.g. 0xdead..beef)
    Provider.prototype.getTxReceipt = function (txHash) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.request("tx_info", [txHash])];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Provider.prototype.getPriorityOpStatus = function (serialId) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.request("ethop_info", [serialId])];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Provider.prototype.notifyPriorityOp = function (serialId, action) {
        return __awaiter(this, void 0, void 0, function () {
            var priorOpStatus, notifyDone;
            var _this = this;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!this.transport.subscriptionsSupported()) return [3 /*break*/, 2];
                        return [4 /*yield*/, new Promise(function (resolve) {
                                var sub = _this.transport.subscribe("ethop_subscribe", [serialId, action], "ethop_unsubscribe", function (resp) {
                                    sub.then(function (sub) { return sub.unsubscribe(); });
                                    resolve(resp);
                                });
                            })];
                    case 1: return [2 /*return*/, _a.sent()];
                    case 2:
                        if (!true) return [3 /*break*/, 7];
                        return [4 /*yield*/, this.getPriorityOpStatus(serialId)];
                    case 3:
                        priorOpStatus = _a.sent();
                        notifyDone = action == "COMMIT"
                            ? priorOpStatus.block && priorOpStatus.block.committed
                            : priorOpStatus.block && priorOpStatus.block.verified;
                        if (!notifyDone) return [3 /*break*/, 4];
                        return [2 /*return*/, priorOpStatus];
                    case 4: return [4 /*yield*/, utils_1.sleep(3000)];
                    case 5:
                        _a.sent();
                        _a.label = 6;
                    case 6: return [3 /*break*/, 2];
                    case 7: return [2 /*return*/];
                }
            });
        });
    };
    Provider.prototype.notifyTransaction = function (hash, action) {
        return __awaiter(this, void 0, void 0, function () {
            var transactionStatus, notifyDone;
            var _this = this;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!this.transport.subscriptionsSupported()) return [3 /*break*/, 2];
                        return [4 /*yield*/, new Promise(function (resolve) {
                                var sub = _this.transport.subscribe("tx_subscribe", [hash, action], "tx_unsubscribe", function (resp) {
                                    sub.then(function (sub) { return sub.unsubscribe(); });
                                    resolve(resp);
                                });
                            })];
                    case 1: return [2 /*return*/, _a.sent()];
                    case 2:
                        if (!true) return [3 /*break*/, 7];
                        return [4 /*yield*/, this.getTxReceipt(hash)];
                    case 3:
                        transactionStatus = _a.sent();
                        notifyDone = action == "COMMIT"
                            ? transactionStatus.block && transactionStatus.block.committed
                            : transactionStatus.block && transactionStatus.block.verified;
                        if (!notifyDone) return [3 /*break*/, 4];
                        return [2 /*return*/, transactionStatus];
                    case 4: return [4 /*yield*/, utils_1.sleep(3000)];
                    case 5:
                        _a.sent();
                        _a.label = 6;
                    case 6: return [3 /*break*/, 2];
                    case 7: return [2 /*return*/];
                }
            });
        });
    };
    Provider.prototype.disconnect = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.transport.disconnect()];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    return Provider;
}());
exports.Provider = Provider;
var ETHProxy = /** @class */ (function () {
    function ETHProxy(ethersProvider, contractAddress) {
        this.ethersProvider = ethersProvider;
        this.contractAddress = contractAddress;
        this.governanceContract = new ethers_1.Contract(this.contractAddress.govContract, utils_1.SYNC_GOV_CONTRACT_INTERFACE, this.ethersProvider);
        this.mainContract = new ethers_1.Contract(this.contractAddress.mainContract, utils_1.SYNC_MAIN_CONTRACT_INTERFACE, this.ethersProvider);
    }
    ETHProxy.prototype.resolveTokenId = function (token) {
        return __awaiter(this, void 0, void 0, function () {
            var tokenId;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        if (!(token == "ETH")) return [3 /*break*/, 1];
                        return [2 /*return*/, 0];
                    case 1: return [4 /*yield*/, this.governanceContract.tokenIds(token)];
                    case 2:
                        tokenId = _a.sent();
                        if (tokenId == 0) {
                            throw new Error("ERC20 token " + token + " is not supported");
                        }
                        return [2 /*return*/, tokenId];
                }
            });
        });
    };
    ETHProxy.prototype.estimateDepositFeeInETHToken = function (token, gasPrice) {
        return __awaiter(this, void 0, void 0, function () {
            var _a, multiplier;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        _a = gasPrice;
                        if (_a) return [3 /*break*/, 2];
                        return [4 /*yield*/, this.ethersProvider.getGasPrice()];
                    case 1:
                        _a = (_b.sent());
                        _b.label = 2;
                    case 2:
                        gasPrice = _a;
                        multiplier = token == "ETH" ? 179000 : 214000;
                        return [2 /*return*/, gasPrice.mul(2 * multiplier)];
                }
            });
        });
    };
    ETHProxy.prototype.estimateEmergencyWithdrawFeeInETHToken = function (gasPrice) {
        return __awaiter(this, void 0, void 0, function () {
            var _a;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0:
                        _a = gasPrice;
                        if (_a) return [3 /*break*/, 2];
                        return [4 /*yield*/, this.ethersProvider.getGasPrice()];
                    case 1:
                        _a = (_b.sent());
                        _b.label = 2;
                    case 2:
                        gasPrice = _a;
                        return [2 /*return*/, gasPrice.mul(2 * 170000)];
                }
            });
        });
    };
    return ETHProxy;
}());
exports.ETHProxy = ETHProxy;
