# Citcuit pseudocode

# Data structures

struct op:
    
    # operation data
    tx_type:        # type of transaction, see the list: https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit#gid=0
    chunk:          # op chunk number (0..3)
    pubdata_chunk:  # current chunk of the pubdata (always 8 bytes)
    args:           # arguments for the operation
    
    # Merkle branch data
    lhs:            # left Merkle branch data
    rhs:            # right Merkle branch data

    # precomputed witness:
    a:              # depends on the optype, used for range checks
    b:              # depends on the optype, used for range checks
    new_root:       # new state root after the operation is applied
    account_path:   # Merkle path witness for the account in the current branch
    subtree_path:   # Merkle path witness for the subtree in the current branch

struct computed:
    last_chunk: bool        # whether the current chunk is the last one in sequence
    pubdata:                # pubdata accumulated over all chunks
    subtractable:           # wheather a >= b
    new_pubkey_hash:        # hash of the new pubkey, truncated to 20 bytes (used only for deposits)
    cosigner_pubkey_hash:   # hash of the cosigner pubkey in the current branch, truncated to 20 bytes


# Circuit functions

def circuit:

    running_hash := initial_hash
    current_root := last_state_root

    prev.lhs := { 0, ... } 
    prev.rhs := { 0, ... } 
    prev.chunk := 0
    prev.new_root := 0

    for op in operations:

        # enfore correct bitlentgh for every input in witness
        # TODO: for this create a macro gadget via struct member annotations
        enforce_bitlength(op)

        enforce_correct_chunking(op, computed)
        accumulate_sha256(op.pubdata_chunk)
        accumulate_pubdata(op, computed)

        # prepare Merkle branch

        cur := select_branch(op, computed)
        full_leaf_index := cur.leaf_is_token << 8 + cur.leaf_index
        computed.cosigner_pubkey_hash := hash(cur.cosigner_pubkey)

        # check initial Merkle paths, before applying the operation

        cur.subtree_root := merkle_root(
            index = full_leaf_index,
            witness = op.subtree_path,
            leaf_data = (cur.leaf_balance, cur.leaf_nonce, cur.creation_nonce, computed.cosigner_pubkey_hash, cur.cosigner_balance, cur.token))
        root := merkle_root(
            index = cur.account, 
            witness = op.account_path,
            leaf_data = hash(cur.owner_pub_key, cur.subtree_root, cur.account_nonce))

        enforce root == current_root

        # check validity and perform state updates for the current branch by modifying `cur` struct

        execute_op(op, cur, computed)

        # check final Merkle paths after applying the operation

        subtree_root := merkle_root(
            index = full_leaf_index,
            witness = op.subtree_path,
            leaf_data = (cur.leaf_balance, cur.leaf_nonce, cur.creation_nonce, computed.cosigner_pubkey_hash, cur.cosigner_balance, cur.token))
        new_root := merkle_root(
            index = cur.account,
            witness = intersection(op.account_path, lhs.account, rhs.account, lhs.intersection_hash, rhs.intersection_hash, current_side),
            leaf_data = hash(cur.owner_pub_key, cur.subtree_root, cur.account_nonce))

        # verify and update root on the last op chunk

        # NOTE: this is checked separately for each branch side, and we already enforced 
        # that `op.new_root` remains unchanged for both by enforcing that it is shared by all chunks
        enforce new_root == op.new_root
        if computed.last_chunk:
            current_root = new_root
        
        # update `prev` references

        # TODO: need a gadget to copy struct members one by one
        prev.rhs = op.rhs
        prev.lhs = op.lhs
        prev.args = op.args
        prev.new_root = op.new_root
        prev.chunk = op.chunk

    # final checks after the loop end

    enforce current_root == new_state_root
    enforce running_hash == pubdata_hash
    enforce last_chunk # any operation should close with the last chunk


# make sure that operation chunks are passed correctly
def enforce_correct_chunking(op, computed):

    # enforce chunk sequence correctness

    enforce (op.chunk == 0) or (op.chunk == prev.chunk + 1) # ensure that chunks come in sequence 
    max_chunks := switch op.tx_type
        deposit => 4,
        transfer_to_new=> 1,
        transfer => 2,
        # ...and so on
        
    enforce op.chunk < max_chunks # 4 constraints
    computed.last_chunk = op.chunk == max_chunks-1 # flag to mark the last op chunk

    # enforce that all chunks share the same witness:
    #   - `op.args` for the common arguments of the operation
    #   - `op.lhs` and `op.rhs` for left and right Merkle branches
    #   - `new_root` of the state after the operation is applied

    correct_inputs := 
        op.chunk == 0 # skip check for the first chunk
        or (
            prev.args == op.args and 
            prev.lhs == op.lhs and 
            prev.rhs == op.rhs and
            prev.new_root == op.new_root
        ) # TODO: need a gadget for logical equality which works with structs
    enforce correct_inputs


# accumulate pubdata from multiple chunks
def accumulate_pubdata(op, computed):
    computed.pubdata =  
        if op.chunk == 0:
            op.pubdata_chunk # initialize from the first chunk
        else:
            computed.pubdata << 8 + op.pubdata_chunk


# determine the Merkle branch side (0 for LHS, 1 for RHS) and set `cur` for the current Merkle branch
def select_branch(op, computed):
    current_side := if op.type == 'deposit': LHS; else: op.chunk

    # TODO: need a gadget for conditional swap applied to each struct member:
    cur := 
        if current_side == LHS: 
            op.lhs; 
        else: 
            op.rhs

    return cur


# verify operation and execute state updates
def execute_op(op, cur, computed):

    # universal range check; a and b are different depending on the op

    computed.subtractable := op.a >= op.b

    # unpack floating point values and hashes

    op.args.amount  := unpack(op.args.amount_packed)
    op.args.fee     := unpack(op.args.fee_packed)

    computed.new_pubkey_hash := hash(cur.new_pubkey) # new pubkey for deposits

    # signature check

    # NOTE: signature check must always be valid, but msg and signer can be phony
    enforce check_sig(cur.sig_msg, cur.signer_pubkey)

    # execute operations

    op_valid := False

    op_valid = op_valid or transfer_to_new(op, cur, computed)
    op_valid = op_valid or deposit(op, cur, computed)
    op_valid = op_valid or full_exit(op, cur, computed)
    op_valid = op_valid or partial_exit(op, cur, computed)
    op_valid = op_valid or escalation(op, cur, computed)
    op_valid = op_valid or op.type == 'noop'

    # `op` MUST be one of the operations and MUST be valid

    enforce op_valid


def transfer_to_new(op, cur, computed):
    # transfer_to_new validation is split into lhs and rhs; pubdata is combined from both branches

    transfer_to_new_lhs :=
        op.type == 'transfer_to_new'

        # here we process the first chunk
        and op.chunk == 0

        # sender is using a token balance, not subaccount
        and lhs.leaf_is_token

        # sender authorized spending and recepient
        and lhs.sig_msg == ('transfer_to_new', lhs.account, lhs.leaf_index, lhs.account_nonce, op.args.amount_packed, op.args.fee_packed, cur.new_pubkey_hash)

        # sender is account owner
        and lhs.signer_pubkey == cur.owner_pub_key

        # sender has enough balance: we checked above that `op.a >= op.b`
        # NOTE: no need to check overflow for `amount + fee` because their bitlengths are enforced]
        and computed.subtractable and (op.a == cur.leaf_balance) and (op.b == (op.args.amount + op.args.fee) )

    # NOTE: updating the state is done by modifying data in the `cur` branch
    if transfer_to_new_lhs:
        cur.leaf_balance = cur.leaf_balance - (op.args.amount + op.args.fee)
        cur.account_nonce = cur.account_nonce + 1

    transfer_to_new_rhs := 
        op.type == 'transfer_to_new'

        # here we process the second (last) chunk
        and op.chunk == 1

        # pubdata contains correct data from both branches, so we verify it agains `lhs` and `rhs`
        and pubdata == (op.tx_type, lhs.account, lhs.leaf_index, lhs.amount, cur.new_pubkey_hash, rhs.account, rhs.fee)

        # new account branch is empty
        and (rhs.owner_pub_key, rhs.subtree_root, rhs.account_nonce) == EMPTY_ACCOUNT

        # deposit is into a token balance, not subaccount
        and rhs.leaf_is_token

        # sender signed the same recepient pubkey of which the hash was passed to public data
        and lhs.new_pubkey == rhs.new_pubkey

    if transfer_to_new_rhs:
        cur.leaf_balance = op.args.amount
    
    return transfer_to_new_lhs or transfer_to_new_rhs


def deposit(op, cur, computed):
    ignore_pubdata := not last_chunk
    deposit := 
        op.type == 'deposit'
        and (ignore_pubdata or pubdata == (cur.account, cur.leaf_index, args.amount, cur.new_pubkey_hash, args.fee))
        and (cur.account_pubkey, cur.subtree_root, cur.account_nonce) == EMPTY_ACCOUNT
        and cur.leaf_is_token
        and computed.subtractable and (op.a == op.args.amount) and (op.b == op.args.fee )

    if deposit:
        cur.leaf_balance = op.args.amount - op.args.fee

    return deposit

def full_exit(op, cur, computed):
    full_exit :=
        op.type == 'full_exit' and
        pubdata == (cur.account, cur.subtree_root)

    if full_exit:
        cur.owner_pub_key = 0
        cur.account_nonce = 0
        cur.subtree_root  = EMPTY_TREE_ROOT

        # NOTE: we also need to clear the balance leaf #0 passed as witness so that subtree_root check passes
        cur.leaf_balance = 0
        cur.leaf_nonce = 0
        cur.creation_nonce = 0
        cur.cosigner_pubkey_hash = EMPTY_HASH
    
    return full_exit


def partial_exit(op, cur, computed):
    partial_exit := 
        op.type == 'partial_exit' and
        pubdata == (op.tx_type, cur.account, cur.leaf_index, op.args.amount, op.args.fee) and
        subtractable and
        cur.leaf_is_token and
        cur.sig_msg == ('partial_exit', cur.account, cur.leaf_index, cur.account_nonce, cur.amount, cur.fee) and
        cur.signer_pubkey == cur.owner_pub_key

    if partial_exit:
        cur.leaf_balance = cur.leaf_balance - (op.args.amount + op.args.fee)
        cur.account_nonce = cur.leaf_nonce + 1
    
    return partial_exit


def escalation(op, cur, computed):
    escalation := 
        op.type == 'escalation' and
        pubdata == (op.tx_type, cur.account, cur.leaf_index, cur.creation_nonce, cur.leaf_nonce) and
        not cur.leaf_is_token and
        cur.sig_msg == ('escalation', cur.account, cur.leaf_index, cur.creation_nonce) and
        (cur.signer_pubkey == cur.owner_pub_key or cur.signer_pubkey == cosigner_pubkey)

    if escalation:
        cur.leaf_balance = 0
        cur.leaf_nonce = 0
        cur.creation_nonce = 0
        cur.cosigner_pubkey_hash = EMPTY_HASH
    
    return escalation
