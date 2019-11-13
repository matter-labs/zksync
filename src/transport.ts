import Axios from "axios";
import WebSocketAsPromised = require("websocket-as-promised");
const W3CWebSocket = require("websocket").w3cwebsocket;

export abstract class AbstractTransport {
    abstract async request(method: string, params): Promise<any>;
    async subscribe(
        subMethod: string,
        subParams,
        unsubMethod: string,
        cb: (data: any) => void
    ): Promise<Subscription> {
        throw new Error("subscription are not supported for this transport");
    }
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

export class HTTPTransport extends AbstractTransport {
    public constructor(public address: string) {
        super();
    }

    // JSON RPC request
    async request(method: string, params = null): Promise<any> {
        const request = {
            id: 1,
            jsonrpc: "2.0",
            method,
            params
        };

        const response = await Axios.post(this.address, request).then(resp => {
            return resp.data;
        });

        if (response.result) {
            return response.result;
        } else if (response.error) {
            throw new JRPCError("JRPC response error", response.error);
        } else {
            throw new Error("Unknown JRPC Error");
        }
    }
}

export class WSTransport extends AbstractTransport {
    ws: WebSocketAsPromised;
    private subscriptionCallback: Map<string, (data: any) => void>;

    private constructor(public address: string) {
        super();
        this.ws = new WebSocketAsPromised(address, {
            createWebSocket: url => new W3CWebSocket(url),
            packMessage: data => JSON.stringify(data),
            unpackMessage: data => JSON.parse(data as string),
            attachRequestId: (data, requestId) =>
                Object.assign({ id: requestId }, data), // attach requestId to message as `id` field
            extractRequestId: data => data && data.id
        });

        this.subscriptionCallback = new Map();

        // Call all subscription callbacks
        this.ws.onUnpackedMessage.addListener(data => {
            if (data.params && data.params.subscription) {
                const params = data.params;
                if (this.subscriptionCallback.has(params.subscription)) {
                    this.subscriptionCallback.get(params.subscription)(
                        params.result
                    );
                }
            }
        });
    }

    static async connect(
        address = "ws://127.0.0.1:3031"
    ): Promise<WSTransport> {
        const transport = new WSTransport(address);
        await transport.ws.open();
        return transport;
    }

    async subscribe(
        subMethod: string,
        subParams,
        unsubMethod: string,
        cb: (data: any) => void
    ): Promise<Subscription> {
        const req = { jsonrpc: "2.0", method: subMethod, params: subParams };
        const sub = await this.ws.sendRequest(req);

        if (sub.error) {
            throw new JRPCError("Subscription failed", sub.error);
        }

        const subId = sub.result;
        this.subscriptionCallback.set(subId, cb);

        const unsubscribe = async () => {
            const unsubRep = await this.ws.sendRequest({
                jsonrpc: "2.0",
                method: unsubMethod,
                params: [subId]
            });
            if (unsubRep.error) {
                throw new JRPCError(
                    `Unsubscribe failed: ${subId}, ${JSON.stringify(
                        unsubRep.error
                    )}`,
                    unsubRep.error
                );
            }
            if (unsubRep.result != true) {
                throw new Error(
                    `Unsubscription failed, returned false: ${subId}`
                );
            }
            this.subscriptionCallback.delete(subId);
        };

        return new Subscription(unsubscribe);
    }

    // JSON RPC request
    async request(method: string, params = null): Promise<any> {
        const request = {
            jsonrpc: "2.0",
            method,
            params
        };

        const response = await this.ws.sendRequest(request);

        if (response.result) {
            return response.result;
        } else if (response.error) {
            throw new JRPCError("JRPC response error", response.error);
        } else {
            throw new Error("Unknown JRPC Error");
        }
    }
}
