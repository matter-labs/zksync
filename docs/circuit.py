# Citcuit pseudocode

def circuit:

    running_hash := initial_hash
    current_root := last_state_root

    prev.lhs := { 0, ... } 
    prev.rhs := { 0, ... } 
    prev.chunk := 0

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

        # enforce that previous chunk had exactly the same inputs

        correct_inputs := 
            op.chunk == 0 # skip check for the first chunk
            or (prev.lhs == op.lhs and prev.rhs == op.rhs) # NOTE: need a gadget for logical equality which works with structs
        enforce correct_inputs

        # accumulate running sha256 hash: `op.pubdata_chunk` is always 8 bytes long

        accumulate_hash((op.type, op.pubdata_chunk))

        # accumulate pubdata

        pubdata :=  if op.chunk == 0:
                        op.pubdata_chunk # initialize at the first chunk
                    else:
                        pubdata << 8 + op.pubdata_chunk

        # determine the Merkle branch side (0 for LHS, 1 for RHS) and set variables for current Merkle branch

        current_side := if op.type == 'deposit': LHS; else: op.chunk
        current := if current_side == LHS: lhs; else: rhs # NOTE: need a gadget for conditional swap applied to each struct member

        # build hashes

        pubkey_hash := hash(current.pubkey) # some pubkey, we will see which one later
        cosigner_pubkey_hash := hash(current.cosigner_pubkey)

        # check Merkle paths before operation begins

        enforce current.leaf_is_token is boolean
        full_leaf_index := current.leaf_is_token << 8 + current.leaf_index

        subtree_root := merkle_root(
            index = full_leaf_index,
            witness = subtree_witness,
            leaf_data = (current.leaf_balance, current.leaf_nonce, current.creation_nonce, current.cosigner_pubkey_hash, current.cosigner_balance, current.token))
        
        root := merkle_root(
            index = current.account, 
            witness = account_witness,
            leaf_data = hash(current.owner_pub_key, subtree_root, current.account_nonce))

        enforce root == current_root

        # check validity and perform state updates by modifying `current` struct

        execute_op(op, current, current_side, pubkey_hash, cosigner_pubkey_hash, pubdata)

        # check final Merkle paths after applying the operation

        subtree_root := merkle_root(
            index = full_leaf_index,
            witness = subtree_witness,
            leaf_data = (current.leaf_balance, current.leaf_nonce, current.creation_nonce, current.cosigner_pubkey_hash, current.cosigner_balance, current.token))

        # here we check intersection, therefore updated account hashes must be provided via witness

        new_hash = hash(current.owner_pub_key, subtree_root, current.account_nonce)
        enforce current.new_hash == new_hash

        root := merkle_root(
            index = current.account, 
            witness = intersection(account_witness, lhs.account, rhs.account, lhs.new_hash, rhs.new_hash, current_side),
            leaf_data = new_hash)
        
        # update `prev` references

        prev.rhs = op.rhs # NOTE: need a gadget to copy struct members one by one
        prev.lhs = op.lhs
        prev.chunk = op.chunk

    # final checks

    enforce current_root == new_state_root
    enforce running_hash == pubdata_hash
    # TODO: check that chunks are closed


def execute_op(op, current, current_side, pubkey_hash, cosigner_pubkey_hash, pubdata):

    # range checks

    subtractable := amount <= leaf_balance

    # check carry from previous transaction

    carry_valid := carry == 0 or optype=='deposit_from' # carry only allowed to be set for deposits
    enforce carry_valid

    if carry:
        (amount, fee, pubkey_hash) = carry

    carry = 0

    # check signature

    check_sig(tx.sig_msg, tx.signer_pubkey) # must always be valid, but msg and signer can be phony

    # validate operations

    deposit_valid := 
        (optype == 'deposit' or optype == 'deposit_from') and
        pubdata == (tx.account, tx.leaf_index, tx.amount, pubkey_hash, tx.fee) and
        (owner_pub_key, subtree_root, account_nonce) == EMPTY_ACCOUNT and
        leaf_is_token

    transfer_to_new_valid := 
        optype == 'transfer_to' and
        pubdata == (tx.account, tx.leaf_index) and
        subtractable and
        leaf_is_token and
        deposit_valid and # same checks as for deposit operation
        sig_msg == ('transfer_to_new', tx.account, leaf_index, account_nonce, tx.amount, tx.fee, pubkey_hash) and
        signer_pubkey == tx.owner_pub_key

    full_exit_valid :=
        optype == 'full_exit' and
        pubdata == (tx.account, tx.subtree_root)

    partial_exit_valid := 
        optype == 'partial_exit' and
        pubdata == (tx.account, tx.leaf_index, tx.amount, tx.fee) and
        subtractable and
        leaf_is_token and
        sig_msg == ('partial_exit', tx.account, tx.leaf_index, account_nonce, tx.amount, tx.fee) and
        signer_pubkey == tx.owner_pub_key

    escalation_valid := 
        optype == 'escalation' and
        pubdata == (tx.account, leaf_index, creation_nonce, leaf_nonce) and
        not leaf_is_token and
        sig_msg == ('escalation', tx.account, leaf_index, creation_nonce) and
        (signer_pubkey == tx.owner_pub_key or signer_pubkey == cosigner_pubkey)

    padding_valid := 
        optype == 'padding'

    tx_valid := 
        deposit_valid or
        transfer_to_new_valid or
        full_exit_valid or
        partial_exit_valid or
        escalation_valid or
        padding_valid

    enforce tx_valid

    # update state

    # NOTE: `if conditon: x = y` is implemented as a binary switch: `x = condition ? y : x`

    if deposit_valid:
        leaf_balance = leaf_balance

    if transfer_to_new_valid:
        leaf_balance = leaf_balance - amount
        account_nonce = account_nonce + 1
        carry = (amount, fee, pubkey_hash)

    if full_exit_valid:
        owner_pub_key = 0
        account_nonce = 0
        subtree_root  = EMPTY_TREE_ROOT

    if partial_exit_valid:
        leaf_balance = leaf_balance - amount
        account_nonce = leaf_nonce + 1

    if escalation_valid:
        leaf_balance = 0
        leaf_nonce = 0
        creation_nonce = 0
        cosigner_pubkey_hash = EMPTY_HASH
