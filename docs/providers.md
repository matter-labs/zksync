# Intro 

JSON-RPC protocol is used to communicate with Sync network nodes.
`SyncProvider` is used to abstract details of the communication and provides useful api for interaction with Sync network.

We support HTTP and WebSocket transport protocol for JSON-RPC communications. WebSocket transport is preferred since it supports subscriptions.
`HTTPTransport` and `WSTransport` classes are used to implement details of communication, but usually you don't need to deal with this 
objects directly.

# API

## SyncProvider

### newWebsocketProvider

#### Signature

```typescript
static async newWebsocketProvider(
        address: string = "ws://127.0.0.1:3031"
    ): Promise<SyncProvider>;
```

#### Parameter

| Name | Description | 
| -- | -- |
| address | Address of the websocket endpoint of a Sync node, starts with `ws://` |
| returns | Sync provider connected to the WebSocket endpoint |
