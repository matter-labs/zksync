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

type PriorityOpInterface<T> = {
    tx_hash: string;
    block_number: number;
    op: T;
    success: boolean;
    fail_reason: null;
    created_at: string;
    batch_id: null;
};

type L2TxInterface<T> = {
    tx_hash: string;
    block_number: number;
    op: T;
    success: boolean;
    fail_reason: string | null;
    created_at: string;
    batch_id: number | null;
};

export type Interface = (
    | PriorityOpInterface<DepositOp>
    | PriorityOpInterface<FullExitOp>
    | L2TxInterface<TransferOp>
    | L2TxInterface<WithdrawOp>
    | L2TxInterface<ChangePubKeyOp>
    | L2TxInterface<ForcedExitOp>
    | L2TxInterface<WithdrawNFTOp>
    | L2TxInterface<MintNFTOp>
    | L2TxInterface<SwapOp>
)[];
