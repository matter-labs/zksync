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
Object.defineProperty(exports, "__esModule", { value: true });
var BN = require("bn.js");
var axios_1 = __importDefault(require("axios"));
var sign_1 = require("./sign");
var crypto_js_1 = require("crypto-js");
var ethers_1 = require("ethers");
var bigNumberify = ethers_1.ethers.utils.bigNumberify;
var IERC20Conract = require("../abi/IERC20");
var franklinContractCode = require("../abi/Franklin");
var FranklinProvider = /** @class */ (function () {
    function FranklinProvider(providerAddress) {
        if (providerAddress === void 0) { providerAddress = 'http://127.0.0.1:3000'; }
        this.providerAddress = providerAddress;
    }
    FranklinProvider.prototype.submitTx = function (tx) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, axios_1.default.post(this.providerAddress + '/api/v0.1/submit_tx', tx).then(function (reps) { return reps.data; })];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getTokens = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, axios_1.default.get(this.providerAddress + '/api/v0.1/tokens').then(function (reps) { return reps.data; })];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    FranklinProvider.prototype.getState = function (address) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, axios_1.default.get(this.providerAddress + '/api/v0.1/account/' + address).then(function (reps) { return reps.data; })];
                    case 1: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    return FranklinProvider;
}());
var Wallet = /** @class */ (function () {
    function Wallet(seed, provider, ethWallet, ethAddress) {
        this.provider = provider;
        this.ethWallet = ethWallet;
        this.ethAddress = ethAddress;
        var privateKey = new BN(crypto_js_1.HmacSHA512(seed.toString('hex'), 'Matter seed').toString(), 'hex');
        this.privateKey = privateKey.mod(sign_1.altjubjubCurve.n);
        this.publicKey = sign_1.altjubjubCurve.g.mul(this.privateKey).normalize();
        var _a = [this.publicKey.getX(), this.publicKey.getY()], x = _a[0], y = _a[1];
        var buff = Buffer.from(x.toString('hex').padStart(64, '0') + y.toString('hex').padStart(64, '0'), 'hex');
        var hash = sign_1.pedersenHash(buff);
        this.address = '0x' + (hash.getX().toString('hex').padStart(64, '0') + hash.getY().toString('hex').padStart(64, '0')).slice(0, 27 * 2);
    }
    Wallet.prototype.depositOnchain = function (token, amount) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract, franklinAddressBinary, tx, erc20DeployedToken, tx;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
                        franklinAddressBinary = Buffer.from(this.address.substr(2), "hex");
                        if (!(token.id == 0)) return [3 /*break*/, 3];
                        return [4 /*yield*/, franklinDeployedContract.depositETH(franklinAddressBinary, { value: amount })];
                    case 1:
                        tx = _a.sent();
                        return [4 /*yield*/, tx.wait(2)];
                    case 2:
                        _a.sent();
                        return [2 /*return*/, tx.hash];
                    case 3:
                        erc20DeployedToken = new ethers_1.Contract(token.address, IERC20Conract.abi, this.ethWallet);
                        return [4 /*yield*/, erc20DeployedToken.approve(franklinDeployedContract.address, amount)];
                    case 4:
                        _a.sent();
                        return [4 /*yield*/, franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, franklinAddressBinary, { gasLimit: bigNumberify("150000") })];
                    case 5:
                        tx = _a.sent();
                        return [4 /*yield*/, tx.wait(2)];
                    case 6:
                        _a.sent();
                        return [2 /*return*/, tx.hash];
                }
            });
        });
    };
    Wallet.prototype.depositOffchain = function (token, amount, fee) {
        return __awaiter(this, void 0, void 0, function () {
            var nonce, tx;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getNonce()];
                    case 1:
                        nonce = _a.sent();
                        tx = {
                            type: 'Deposit',
                            to: this.address,
                            token: token.id,
                            amount: bigNumberify(amount).toString(),
                            fee: bigNumberify(fee).toString(),
                            nonce: nonce,
                        };
                        return [4 /*yield*/, this.provider.submitTx(tx)];
                    case 2: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Wallet.prototype.widthdrawOnchain = function (token, amount) {
        return __awaiter(this, void 0, void 0, function () {
            var franklinDeployedContract, tx, tx;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        franklinDeployedContract = new ethers_1.Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
                        if (!(token.id == 0)) return [3 /*break*/, 3];
                        return [4 /*yield*/, franklinDeployedContract.withdrawETH(amount, { gasLimit: 200000 })];
                    case 1:
                        tx = _a.sent();
                        return [4 /*yield*/, tx.wait(2)];
                    case 2:
                        _a.sent();
                        return [2 /*return*/, tx.hash];
                    case 3: return [4 /*yield*/, franklinDeployedContract.withdrawERC20(token.address, amount, { gasLimit: bigNumberify("150000") })];
                    case 4:
                        tx = _a.sent();
                        return [4 /*yield*/, tx.wait(2)];
                    case 5:
                        _a.sent();
                        return [2 /*return*/, tx.hash];
                }
            });
        });
    };
    Wallet.prototype.widthdrawOffchain = function (token, amount, fee) {
        return __awaiter(this, void 0, void 0, function () {
            var nonce, tx, _a;
            return __generator(this, function (_b) {
                switch (_b.label) {
                    case 0: return [4 /*yield*/, this.getNonce()];
                    case 1:
                        nonce = _b.sent();
                        _a = {
                            type: 'Withdraw',
                            account: this.address
                        };
                        return [4 /*yield*/, this.ethWallet.getAddress()];
                    case 2:
                        tx = (_a.eth_address = _b.sent(),
                            _a.token = token.id,
                            _a.amount = bigNumberify(amount).toString(),
                            _a.fee = bigNumberify(fee).toString(),
                            _a.nonce = nonce,
                            _a);
                        return [4 /*yield*/, this.provider.submitTx(tx)];
                    case 3: return [2 /*return*/, _b.sent()];
                }
            });
        });
    };
    Wallet.prototype.transfer = function (address, token, amount, fee) {
        return __awaiter(this, void 0, void 0, function () {
            var nonce, tx;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.getNonce()];
                    case 1:
                        nonce = _a.sent();
                        tx = {
                            type: 'Transfer',
                            from: this.address,
                            to: address,
                            token: token.id,
                            amount: bigNumberify(amount).toString(),
                            fee: bigNumberify(fee).toString(),
                            nonce: nonce,
                        };
                        return [4 /*yield*/, this.provider.submitTx(tx)];
                    case 2: return [2 /*return*/, _a.sent()];
                }
            });
        });
    };
    Wallet.prototype.getNonce = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.fetchFranklinState()];
                    case 1:
                        _a.sent();
                        return [2 /*return*/, this.franklinState.commited.nonce];
                }
            });
        });
    };
    Wallet.fromEthWallet = function (wallet) {
        return __awaiter(this, void 0, void 0, function () {
            var defaultFranklinProvider, seed, ethAddress, frankinWallet;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        defaultFranklinProvider = new FranklinProvider();
                        return [4 /*yield*/, wallet.signMessage('Matter login')];
                    case 1:
                        seed = (_a.sent()).substr(2);
                        return [4 /*yield*/, wallet.getAddress()];
                    case 2:
                        ethAddress = _a.sent();
                        frankinWallet = new Wallet(Buffer.from(seed, 'hex'), defaultFranklinProvider, wallet, ethAddress);
                        return [2 /*return*/, frankinWallet];
                }
            });
        });
    };
    Wallet.prototype.fetchEthState = function () {
        return __awaiter(this, void 0, void 0, function () {
            var onchainBalances, contractBalances, lockedBlocksLeft, currentBlock, franklinDeployedContract, _i, _a, token, _b, _c, erc20DeployedToken, _d, _e, balanceStorage;
            return __generator(this, function (_f) {
                switch (_f.label) {
                    case 0:
                        onchainBalances = new Array(this.supportedTokens.length);
                        contractBalances = new Array(this.supportedTokens.length);
                        lockedBlocksLeft = new Array(this.supportedTokens.length);
                        return [4 /*yield*/, this.ethWallet.provider.getBlockNumber()];
                    case 1:
                        currentBlock = _f.sent();
                        franklinDeployedContract = new ethers_1.Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
                        _i = 0, _a = this.supportedTokens;
                        _f.label = 2;
                    case 2:
                        if (!(_i < _a.length)) return [3 /*break*/, 9];
                        token = _a[_i];
                        if (!(token.id == 0)) return [3 /*break*/, 4];
                        _b = onchainBalances;
                        _c = token.id;
                        return [4 /*yield*/, this.ethWallet.provider.getBalance(this.ethAddress)];
                    case 3:
                        _b[_c] = _f.sent();
                        return [3 /*break*/, 6];
                    case 4:
                        erc20DeployedToken = new ethers_1.Contract(token.address, IERC20Conract.abi, this.ethWallet);
                        _d = onchainBalances;
                        _e = token.id;
                        return [4 /*yield*/, erc20DeployedToken.balanceOf(this.ethAddress).then(function (n) { return n.toString(); })];
                    case 5:
                        _d[_e] = _f.sent();
                        _f.label = 6;
                    case 6: return [4 /*yield*/, franklinDeployedContract.balances(this.ethAddress, token.id)];
                    case 7:
                        balanceStorage = _f.sent();
                        contractBalances[token.id] = balanceStorage.balance;
                        lockedBlocksLeft[token.id] = Math.max(balanceStorage.lockedUntilBlock - currentBlock, 0);
                        _f.label = 8;
                    case 8:
                        _i++;
                        return [3 /*break*/, 2];
                    case 9:
                        this.ethState = { onchainBalances: onchainBalances, contractBalances: contractBalances, lockedBlocksLeft: lockedBlocksLeft };
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
