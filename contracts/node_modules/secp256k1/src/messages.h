#ifndef _SECP256K1_NODE_MESSAGES_
# define _SECP256K1_NODE_MESSAGES_

#define COMPRESSED_TYPE_INVALID "compressed should be a boolean"

#define EC_PRIVATE_KEY_TYPE_INVALID "private key should be a Buffer"
#define EC_PRIVATE_KEY_LENGTH_INVALID "private key length is invalid"
#define EC_PRIVATE_KEY_RANGE_INVALID "private key range is invalid"
#define EC_PRIVATE_KEY_TWEAK_ADD_FAIL "tweak out of range or resulting private key is invalid"
#define EC_PRIVATE_KEY_TWEAK_MUL_FAIL "tweak out of range"
#define EC_PRIVATE_KEY_EXPORT_DER_FAIL "couldn't export to DER format"
#define EC_PRIVATE_KEY_IMPORT_DER_FAIL "couldn't import from DER format"

#define EC_PUBLIC_KEYS_TYPE_INVALID "public keys should be an Array"
#define EC_PUBLIC_KEYS_LENGTH_INVALID "public keys Array should have at least 1 element"
#define EC_PUBLIC_KEY_TYPE_INVALID "public key should be a Buffer"
#define EC_PUBLIC_KEY_LENGTH_INVALID "public key length is invalid"
#define EC_PUBLIC_KEY_PARSE_FAIL "the public key could not be parsed or is invalid"
#define EC_PUBLIC_KEY_CREATE_FAIL "private was invalid, try again"
#define EC_PUBLIC_KEY_TWEAK_ADD_FAIL "tweak out of range or resulting public key is invalid"
#define EC_PUBLIC_KEY_TWEAK_MUL_FAIL "tweak out of range"
#define EC_PUBLIC_KEY_COMBINE_FAIL "the sum of the public keys is not valid"

#define ECDH_FAIL "scalar was invalid (zero or overflow)"

#define ECDSA_SIGNATURE_TYPE_INVALID "signature should be a Buffer"
#define ECDSA_SIGNATURE_LENGTH_INVALID "signature length is invalid"
#define ECDSA_SIGNATURE_PARSE_FAIL "couldn't parse signature"
#define ECDSA_SIGNATURE_PARSE_DER_FAIL "couldn't parse DER signature"
#define ECDSA_SIGNATURE_SERIALIZE_DER_FAIL "couldn't serialize signature to DER format"

#define ECDSA_SIGN_FAIL "nonce generation function failed or private key is invalid"
#define ECDSA_RECOVER_FAIL "couldn't recover public key from signature"

#define MSG32_TYPE_INVALID "message should be a Buffer"
#define MSG32_LENGTH_INVALID "message length is invalid"

#define OPTIONS_TYPE_INVALID "options should be an Object"
#define OPTIONS_DATA_TYPE_INVALID "options.data should be a Buffer"
#define OPTIONS_DATA_LENGTH_INVALID "options.data length is invalid"
#define OPTIONS_NONCEFN_TYPE_INVALID "options.noncefn should be a Function"

#define RECOVERY_ID_TYPE_INVALID "recovery should be a Number"
#define RECOVERY_ID_VALUE_INVALID "recovery should have value between -1 and 4"

#define TWEAK_TYPE_INVALID "tweak should be a Buffer"
#define TWEAK_LENGTH_INVALID "tweak length is invalid"

#endif
