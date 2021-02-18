import { BigNumber } from 'ethers';
import * as ethers from 'ethers';
import Axios from 'axios';
import WebSocketAsPromised = require('websocket-as-promised');
import * as websocket from 'websocket';
import { PubKeyHash } from './types';
import { Signer } from './signer';

const W3CWebSocket = websocket.w3cwebsocket;

export abstract class AbstractJSONRPCTransport {
    abstract request(method: string, params): Promise<any>;
    subscriptionsSupported(): boolean {
        return false;
    }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    async subscribe(subMethod: string, subParams, unsubMethod: string, cb: (data: any) => void): Promise<Subscription> {
        throw new Error('subscription are not supported for this transport');
    }
    abstract disconnect();
}

// Has jrpcError field which is JRPC error object.
// https://www.jsonrpc.org/specification#error_object
export class JRPCError extends Error {
    constructor(message: string, public jrpcError: JRPCErrorObject) {
        super(message);
    }
}
export interface JRPCErrorObject {
    code: number;
    message: string;
    data: any;
}

class Subscription {
    constructor(public unsubscribe: () => Promise<void>) {}
}

export class HTTPTransport extends AbstractJSONRPCTransport {
    public constructor(public address: string) {
        super();
    }

    // JSON RPC request
    async request(method: string, params = null): Promise<any> {
        const request = {
            id: 1,
            jsonrpc: '2.0',
            method,
            params
        };

        const response = await Axios.post(this.address, request).then((resp) => {
            return resp.data;
        });

        if ('result' in response) {
            return response.result;
        } else if ('error' in response) {
            throw new JRPCError(
                `zkSync API response error: code ${response.error.code}; message: ${response.error.message}`,
                response.error
            );
        } else {
            throw new Error('Unknown JRPC Error');
        }
    }

    async disconnect() {}
}

/**
 * @deprecated Websocket support will be removed in future. Use HTTP transport instead.
 */
export class WSTransport extends AbstractJSONRPCTransport {
    ws: WebSocketAsPromised;
    private subscriptionCallback: Map<string, (data: any) => void>;

    private constructor(public address: string) {
        super();
        this.ws = new WebSocketAsPromised(address, {
            createWebSocket: (url) => new W3CWebSocket(url),
            packMessage: (data) => JSON.stringify(data),
            unpackMessage: (data) => JSON.parse(data as string),
            attachRequestId: (data, requestId) => Object.assign({ id: requestId }, data), // attach requestId to message as `id` field
            extractRequestId: (data) => data && data.id
        });

        this.subscriptionCallback = new Map();

        // Call all subscription callbacks
        this.ws.onUnpackedMessage.addListener((data) => {
            if (data.params && data.params.subscription) {
                const params = data.params;
                if (this.subscriptionCallback.has(params.subscription)) {
                    this.subscriptionCallback.get(params.subscription)(params.result);
                }
            }
        });
    }

    static async connect(address = 'ws://127.0.0.1:3031'): Promise<WSTransport> {
        const transport = new WSTransport(address);
        await transport.ws.open();
        return transport;
    }

    subscriptionsSupported(): boolean {
        return true;
    }

    async subscribe(subMethod: string, subParams, unsubMethod: string, cb: (data: any) => void): Promise<Subscription> {
        const req = { jsonrpc: '2.0', method: subMethod, params: subParams };
        const sub = await this.ws.sendRequest(req);

        if (sub.error) {
            throw new JRPCError('Subscription failed', sub.error);
        }

        const subId = sub.result;
        this.subscriptionCallback.set(subId, cb);

        const unsubscribe = async () => {
            const unsubRep = await this.ws.sendRequest({
                jsonrpc: '2.0',
                method: unsubMethod,
                params: [subId]
            });
            if (unsubRep.error) {
                throw new JRPCError(`Unsubscribe failed: ${subId}, ${JSON.stringify(unsubRep.error)}`, unsubRep.error);
            }
            if (unsubRep.result !== true) {
                throw new Error(`Unsubscription failed, returned false: ${subId}`);
            }
            this.subscriptionCallback.delete(subId);
        };

        return new Subscription(unsubscribe);
    }

    // JSON RPC request
    async request(method: string, params = null): Promise<any> {
        const request = {
            jsonrpc: '2.0',
            method,
            params
        };

        const response = await this.ws.sendRequest(request, { requestId: 1 });

        if ('result' in response) {
            return response.result;
        } else if ('error' in response) {
            throw new JRPCError(
                `zkSync API response error: code ${response.error.code}; message: ${response.error.message}`,
                response.error
            );
        } else {
            throw new Error('Unknown JRPC Error');
        }
    }

    async disconnect() {
        await this.ws.close();
    }
}

export class DummyTransport extends AbstractJSONRPCTransport {
    public constructor(public network: string, public ethPrivateKey: Uint8Array, public getTokens: Function) {
        super();
    }

    async getPubKeyHash(): Promise<PubKeyHash> {
        const ethWallet = new ethers.Wallet(this.ethPrivateKey);
        const { signer } = await Signer.fromETHSignature(ethWallet);
        return await signer.pubKeyHash();
    }

    async request(method: string, params = null): Promise<any> {
        if (method == 'contract_address') {
            return {
                // The HEX-encoded sequence of bytes [0..20) provided as the `mainContract`.
                mainContract: '0x000102030405060708090a0b0c0d0e0f10111213',
                //  The `govContract` is not used in tests and it is simply an empty string.
                govContract: ''
            };
        }

        if (method == 'tokens') {
            const tokensList = this.getTokens();
            const tokens = {};

            let id = 1;
            for (const tokenItem of tokensList.slice(0, 3)) {
                const token = {
                    address: tokenItem.address,
                    id: id,
                    symbol: tokenItem.symbol,
                    decimals: tokenItem.decimals
                };

                tokens[tokenItem.symbol] = token;
                id++;
            }

            return tokens;
        }

        if (method == 'account_info') {
            // The example `AccountState` instance:
            //  - assigns the '42' value to account_id;
            //  - assigns the committed.pubKeyHash to match the wallet's signer's PubKeyHash
            //  - adds single entry of "DAI" token to the committed balances;
            //  - adds single entry of "USDC" token to the verified balances.
            return {
                address: params[0],
                id: 42,
                depositing: {},
                committed: {
                    balances: {
                        DAI: BigNumber.from(12345)
                    },
                    nonce: 0,
                    pubKeyHash: await this.getPubKeyHash()
                },
                verified: {
                    balances: {
                        USDC: BigNumber.from(98765)
                    },
                    nonce: 0,
                    pubKeyHash: ''
                }
            };
        }

        return {
            method,
            params
        };
    }

    async disconnect() {}
}
