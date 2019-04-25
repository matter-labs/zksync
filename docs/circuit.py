# Citcuit pseudocode

def circuit:

    running_hash := initial_hash
    current_root := last_state_root

    prev.lhs := { 0, ... } 
    prev.rhs := { 0, ... } 
    prev.chunk := 0
    prev.new_root := 0
    last_chunk := true

    for op in operations:

        # enforce opcode correctness

        enforce op.chunk < 4
        enforce op.type < 16
        enforce op.opcode == op.chunk * 0x10 + op.type

        # enforce correct chunk sequence

        enforce (op.chunk == 0) or (op.chunk == prev.chunk + 1) # ensure that chunks come in sequence 
        max_chunks := switch op.type
            'deposit' => 4,
            'transfer_to_new'=> 1,
            'transfer' => 2,
            # ...
        enforce op.chunk < max_chunks # 4 constraints
        last_chunk = op.chunk == max_chunks-1 # flag to mark the last op chunk

        # enforce that all chunks share the same witness:
        #   - `args` for the op arguments
        #   - `lhs` and `rhs` for data of leaves involved in the operation
        #   - `new_root` of the state after the op is applied

        correct_inputs := 
            op.chunk == 0 # skip check for the first chunk
            or (
                prev.args == op.args and 
                prev.lhs == op.lhs and 
                prev.rhs == op.rhs and
                prev.new_root == op.new_root
            ) # NOTE: need a gadget for logical equality which works with structs
        enforce correct_inputs

        # accumulate running sha256 hash: `op.pubdata_chunk` is always 8 bytes long

        accumulate_hash((op.type, op.pubdata_chunk))

        # accumulate pubdata

        pubdata :=  if op.chunk == 0:
                        op.pubdata_chunk # initialize at the first chunk
                    else:
                        pubdata << 8 + op.pubdata_chunk

        # determine the Merkle branch side (0 for LHS, 1 for RHS) and set variables for the current Merkle branch

        current_side := if op.type == 'deposit': LHS; else: op.chunk
        cur := if current_side == LHS: lhs; else: rhs # NOTE: need a gadget for conditional swap applied to each struct member

        # build hashes for data in the current branch

        cur.pubkey_hash := hash(cur.pubkey) # some pubkey, we will see which one later
        cur.cosigner_pubkey_hash := hash(cur.cosigner_pubkey)

        # check Merkle paths before operation begins

        enforce cur.leaf_is_token is boolean
        full_leaf_index := cur.leaf_is_token << 8 + cur.leaf_index

        cur.subtree_root := merkle_root(
            index = full_leaf_index,
            witness = subtree_witness,
            leaf_data = (cur.leaf_balance, cur.leaf_nonce, cur.creation_nonce, cur.cosigner_pubkey_hash, cur.cosigner_balance, cur.token))
        
        root := merkle_root(
            index = cur.account, 
            witness = account_witness,
            leaf_data = hash(cur.owner_pub_key, subtree_root, cur.account_nonce))

        enforce root == current_root

        # check validity and perform state updates by modifying `cur` struct

        execute_op(op, cur, lhs, rhs, pubdata, last_chunk)

        # check final Merkle paths after applying the operation

        subtree_root := merkle_root(
            index = full_leaf_index,
            witness = subtree_witness,
            leaf_data = (cur.leaf_balance, cur.leaf_nonce, cur.creation_nonce, cur.cosigner_pubkey_hash, cur.cosigner_balance, cur.token))

        new_hash = hash(cur.owner_pub_key, cur.subtree_root, cur.account_nonce)
        enforce cur.new_hash == new_hash # NOTE: we check intersection below, therefore updated account hashes must be provided via witness

        new_root := merkle_root(
            index = cur.account, 
            witness = intersection(account_witness, lhs.account, rhs.account, lhs.new_hash, rhs.new_hash, current_side),
            leaf_data = new_hash)

        # verify and update root on last chunk

        enforce new_root == op.new_root # NOTE: we already enforced above that `op.new_root` remains unchanged for all chunks

        if last_chunk:
            current_root = new_root
        
        # update `prev` references

        prev.rhs = op.rhs # NOTE: need a gadget to copy struct members one by one
        prev.lhs = op.lhs
        prev.args = op.args
        prev.new_root = op.new_root
        prev.chunk = op.chunk

    # final checks

    enforce current_root == new_state_root
    enforce running_hash == pubdata_hash
    enforce last_chunk


def execute_op(op, cur, lhs, rhs, pubdata, last_chunk):

    # range checks: no need to check overflow for `amount + fee` because their bitsize is enforced via sha256 running hash

    subtractable := (op.args.amount + op.args.fee) <= cur.leaf_balance and op.args.amount >= op.args.fee

    # check signature

    check_sig(cur.sig_msg, cur.signer_pubkey) # must always be valid, but msg and signer can be phony
    
    # transfer_to_new validation is split into lhs and rhs; pubdata is combined from both branches

    transfer_to_new_lhs :=
        op.type == 'transfer_to_new'

        # here we process the first chunk
        and op.chunk == 0

        # sender is using a token balance, not subaccount
        and lhs.leaf_is_token

        # sender authorized spending and recepient
        and lhs.sig_msg == ('transfer_to_new', lhs.account, lhs.leaf_index, lhs.account_nonce, op.args.amount, op.args.fee, cur.pubkey_hash)

        # sender is account owner
        and lhs.signer_pubkey == cur.owner_pub_key

        # sender has enough balance
        and subtractable

    transfer_to_new_rhs := 
        op.type == 'transfer_to_new'

        # here we process the second (last) chunk
        and op.chunk == 1

        # pubdata contains correct data from both branches, so we verify it agains `lhs` and `rhs`
        and pubdata == (lhs.account, lhs.leaf_index, lhs.amount, cur.pubkey_hash, rhs.account, rhs.fee)

        # sender signed the same recepient pubkey of which the hash passed to public data
        and lhs.pubkey == rhs.pubkey

        # leaf of the new account is empty
        and (rhs.owner_pub_key, rhs.subtree_root, rhs.account_nonce) == EMPTY_ACCOUNT

        # deposit into a token balance, not subaccount
        and rhs.leaf_is_token

    # following operations are of 1 chunk, so `lhs` and `rhs` are not used since we only need to check data in the current branch

    ignore_pubdata := not last_chunk
    deposit := 
        (op.type == 'deposit' or op.type == 'deposit_from') and
        (ignore_pubdata or pubdata == (cur.account, cur.leaf_index, args.amount, cur.pubkey_hash, args.fee)) and
        (cur.account_pubkey, cur.subtree_root, cur.account_nonce) == EMPTY_ACCOUNT and
        cur.leaf_is_token

    full_exit :=
        op.type == 'full_exit' and
        pubdata == (cur.account, cur.subtree_root)

    partial_exit := 
        op.type == 'partial_exit' and
        pubdata == (cur.account, cur.leaf_index, op.args.amount, op.args.fee) and
        subtractable and
        cur.leaf_is_token and
        cur.sig_msg == ('partial_exit', cur.account, cur.leaf_index, cur.account_nonce, cur.amount, cur.fee) and
        cur.signer_pubkey == cur.owner_pub_key

    escalation := 
        op.type == 'escalation' and
        pubdata == (cur.account, cur.leaf_index, cur.creation_nonce, cur.leaf_nonce) and
        not cur.leaf_is_token and
        cur.sig_msg == ('escalation', cur.account, cur.leaf_index, cur.creation_nonce) and
        (cur.signer_pubkey == cur.owner_pub_key or cur.signer_pubkey == cosigner_pubkey)

    # noop is always valid, as long as it is a noop! :)

    noop := 
        op.type == 'noop'

    # one of the operations MUST be valid

    tx_valid := 
        deposit or
        transfer_to_new_lhs or 
        transfer_to_new_rhs or
        full_exit or
        partial_exit or
        escalation or
        padding

    enforce tx_valid

    # updating the state is done by modifying data in `cur` branch

    if transfer_to_new_lhs:
        cur.leaf_balance = cur.leaf_balance - (op.args.amount + op.args.fee)
        cur.account_nonce = cur.account_nonce + 1

    if transfer_to_new_rhs:
        cur.leaf_balance = op.args.amount - op.args.fee

    if deposit:
        cur.leaf_balance = op.args.amount - op.args.fee

    if full_exit:
        cur.owner_pub_key = 0
        cur.account_nonce = 0
        cur.subtree_root  = EMPTY_TREE_ROOT

        # we also need to clear the balance leaf #0 passed as witness so that subtree_root check passes
        cur.leaf_balance = 0
        cur.leaf_nonce = 0
        cur.creation_nonce = 0
        cur.cosigner_pubkey_hash = EMPTY_HASH

    if partial_exit:
        cur.leaf_balance = cur.leaf_balance - (op.args.amount + op.args.fee)
        cur.account_nonce = cur.leaf_nonce + 1

    if escalation:
        cur.leaf_balance = 0
        cur.leaf_nonce = 0
        cur.creation_nonce = 0
        cur.cosigner_pubkey_hash = EMPTY_HASH

