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

type Transfer = {
    tx_hash: string;
    block_number: number;
    op: TransferOp;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

type Deposit = {
    tx_hash: string;
    block_number: number;
    op: DepositOp;
    success: boolean;
    fail_reason: null;
    created_at: string;
    batch_id: null;
};

type ChangePubKey = {
    tx_hash: string;
    block_number: number;
    op: ChangePubKeyOp;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

type Withdraw = {
    tx_hash: string;
    block_number: number;
    op: WithdrawOp;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

type FullExit = {
    tx_hash: string;
    block_number: number;
    op: FullExitOp;
    success: boolean;
    fail_reason: null;
    created_at: string;
    batch_id: null;
};

type ForcedExit = {
    tx_hash: string;
    block_number: number;
    op: ForcedExitOp;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

type WithdrawNFT = {
    tx_hash: string;
    block_number: number;
    op: WithdrawNFTOp;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

type MintNFT = {
    tx_hash: string;
    block_number: number;
    op: MintNFTOp;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

type Swap = {
    tx_hash: string;
    block_number: number;
    op: SwapOp;
    success: boolean;
    fail_reason: string | null;
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
    | WithdrawNFT
    | MintNFT
    | Swap
)[];
