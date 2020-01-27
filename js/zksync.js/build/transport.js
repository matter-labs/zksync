"use strict";
var __extends = (this && this.__extends) || (function () {
    var extendStatics = function (d, b) {
        extendStatics = Object.setPrototypeOf ||
            ({ __proto__: [] } instanceof Array && function (d, b) { d.__proto__ = b; }) ||
            function (d, b) { for (var p in b) if (b.hasOwnProperty(p)) d[p] = b[p]; };
        return extendStatics(d, b);
    };
    return function (d, b) {
        extendStatics(d, b);
        function __() { this.constructor = d; }
        d.prototype = b === null ? Object.create(b) : (__.prototype = b.prototype, new __());
    };
})();
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
var axios_1 = __importDefault(require("axios"));
var WebSocketAsPromised = require("websocket-as-promised");
var W3CWebSocket = require("websocket").w3cwebsocket;
var AbstractJSONRPCTransport = /** @class */ (function () {
    function AbstractJSONRPCTransport() {
    }
    AbstractJSONRPCTransport.prototype.subscriptionsSupported = function () {
        return false;
    };
    AbstractJSONRPCTransport.prototype.subscribe = function (subMethod, subParams, unsubMethod, cb) {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                throw new Error("subscription are not supported for this transport");
            });
        });
    };
    return AbstractJSONRPCTransport;
}());
exports.AbstractJSONRPCTransport = AbstractJSONRPCTransport;
// Has jrpcError field which is JRPC error object.
// https://www.jsonrpc.org/specification#error_object
var JRPCError = /** @class */ (function (_super) {
    __extends(JRPCError, _super);
    function JRPCError(message, jrpcError) {
        var _this = _super.call(this, message) || this;
        _this.jrpcError = jrpcError;
        return _this;
    }
    return JRPCError;
}(Error));
exports.JRPCError = JRPCError;
var Subscription = /** @class */ (function () {
    function Subscription(unsubscribe) {
        this.unsubscribe = unsubscribe;
    }
    return Subscription;
}());
var HTTPTransport = /** @class */ (function (_super) {
    __extends(HTTPTransport, _super);
    function HTTPTransport(address) {
        var _this = _super.call(this) || this;
        _this.address = address;
        return _this;
    }
    // JSON RPC request
    HTTPTransport.prototype.request = function (method, params) {
        if (params === void 0) { params = null; }
        return __awaiter(this, void 0, void 0, function () {
            var request, response;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        request = {
                            id: 1,
                            jsonrpc: "2.0",
                            method: method,
                            params: params
                        };
                        return [4 /*yield*/, axios_1.default.post(this.address, request).then(function (resp) {
                                return resp.data;
                            })];
                    case 1:
                        response = _a.sent();
                        if (response.result) {
                            return [2 /*return*/, response.result];
                        }
                        else if (response.error) {
                            throw new JRPCError("JRPC response error", response.error);
                        }
                        else {
                            throw new Error("Unknown JRPC Error");
                        }
                        return [2 /*return*/];
                }
            });
        });
    };
    HTTPTransport.prototype.disconnect = function () {
        return __awaiter(this, void 0, void 0, function () { return __generator(this, function (_a) {
            return [2 /*return*/];
        }); });
    };
    return HTTPTransport;
}(AbstractJSONRPCTransport));
exports.HTTPTransport = HTTPTransport;
var WSTransport = /** @class */ (function (_super) {
    __extends(WSTransport, _super);
    function WSTransport(address) {
        var _this = _super.call(this) || this;
        _this.address = address;
        _this.ws = new WebSocketAsPromised(address, {
            createWebSocket: function (url) { return new W3CWebSocket(url); },
            packMessage: function (data) { return JSON.stringify(data); },
            unpackMessage: function (data) { return JSON.parse(data); },
            attachRequestId: function (data, requestId) {
                return Object.assign({ id: requestId }, data);
            },
            extractRequestId: function (data) { return data && data.id; }
        });
        _this.subscriptionCallback = new Map();
        // Call all subscription callbacks
        _this.ws.onUnpackedMessage.addListener(function (data) {
            if (data.params && data.params.subscription) {
                var params = data.params;
                if (_this.subscriptionCallback.has(params.subscription)) {
                    _this.subscriptionCallback.get(params.subscription)(params.result);
                }
            }
        });
        return _this;
    }
    WSTransport.connect = function (address) {
        if (address === void 0) { address = "ws://127.0.0.1:3031"; }
        return __awaiter(this, void 0, void 0, function () {
            var transport;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        transport = new WSTransport(address);
                        return [4 /*yield*/, transport.ws.open()];
                    case 1:
                        _a.sent();
                        return [2 /*return*/, transport];
                }
            });
        });
    };
    WSTransport.prototype.subscriptionsSupported = function () {
        return true;
    };
    WSTransport.prototype.subscribe = function (subMethod, subParams, unsubMethod, cb) {
        return __awaiter(this, void 0, void 0, function () {
            var req, sub, subId, unsubscribe;
            var _this = this;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        req = { jsonrpc: "2.0", method: subMethod, params: subParams };
                        return [4 /*yield*/, this.ws.sendRequest(req)];
                    case 1:
                        sub = _a.sent();
                        if (sub.error) {
                            throw new JRPCError("Subscription failed", sub.error);
                        }
                        subId = sub.result;
                        this.subscriptionCallback.set(subId, cb);
                        unsubscribe = function () { return __awaiter(_this, void 0, void 0, function () {
                            var unsubRep;
                            return __generator(this, function (_a) {
                                switch (_a.label) {
                                    case 0: return [4 /*yield*/, this.ws.sendRequest({
                                            jsonrpc: "2.0",
                                            method: unsubMethod,
                                            params: [subId]
                                        })];
                                    case 1:
                                        unsubRep = _a.sent();
                                        if (unsubRep.error) {
                                            throw new JRPCError("Unsubscribe failed: " + subId + ", " + JSON.stringify(unsubRep.error), unsubRep.error);
                                        }
                                        if (unsubRep.result != true) {
                                            throw new Error("Unsubscription failed, returned false: " + subId);
                                        }
                                        this.subscriptionCallback.delete(subId);
                                        return [2 /*return*/];
                                }
                            });
                        }); };
                        return [2 /*return*/, new Subscription(unsubscribe)];
                }
            });
        });
    };
    // JSON RPC request
    WSTransport.prototype.request = function (method, params) {
        if (params === void 0) { params = null; }
        return __awaiter(this, void 0, void 0, function () {
            var request, response;
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0:
                        request = {
                            jsonrpc: "2.0",
                            method: method,
                            params: params
                        };
                        return [4 /*yield*/, this.ws.sendRequest(request)];
                    case 1:
                        response = _a.sent();
                        if (response.result) {
                            return [2 /*return*/, response.result];
                        }
                        else if (response.error) {
                            throw new JRPCError("JRPC response error", response.error);
                        }
                        else {
                            throw new Error("Unknown JRPC Error");
                        }
                        return [2 /*return*/];
                }
            });
        });
    };
    WSTransport.prototype.disconnect = function () {
        return __awaiter(this, void 0, void 0, function () {
            return __generator(this, function (_a) {
                switch (_a.label) {
                    case 0: return [4 /*yield*/, this.ws.close()];
                    case 1:
                        _a.sent();
                        return [2 /*return*/];
                }
            });
        });
    };
    return WSTransport;
}(AbstractJSONRPCTransport));
exports.WSTransport = WSTransport;
