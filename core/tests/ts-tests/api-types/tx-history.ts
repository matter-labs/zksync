import {
    TransferOp,
    WithdrawOp,
    WithdrawNFTOp,
    ChangePubKeyOp,
    FullExitOp,
    ForcedExitOp,
    MintNFTOp,
    SwapOp,
    DepositOp
} from './transaction';

type Deposit = {
    tx_id: string;
    hash: string;
    eth_block: number;
    pq_id: number;
    tx: DepositOp;
    success: boolean;
    fail_reason: null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: null;
};

type FullExit = {
    tx_id: string;
    hash: string;
    eth_block: number;
    pq_id: number;
    tx: FullExitOp;
    success: boolean;
    fail_reason: null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: null;
};

type Transfer = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: TransferOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type ChangePubKey = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: ChangePubKeyOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type Withdraw = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: WithdrawOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type ForcedExit = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: ForcedExitOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type MintNFT = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: MintNFTOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type WithdrawNFT = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: WithdrawNFTOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

type Swap = {
    tx_id: string;
    hash: string;
    eth_block: null;
    pq_id: null;
    tx: SwapOp;
    success: boolean;
    fail_reason: string | null;
    commited: boolean;
    verified: boolean;
    created_at: string;
    batch_id: number | null;
};

export type Interface = (
    | Deposit
    | Transfer
    | Withdraw
    | ChangePubKey
    | FullExit
    | ForcedExit
    | MintNFT
    | WithdrawNFT
    | Swap
)[];
