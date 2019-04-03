#ifndef _SECP256K1_NODE_ECDSA_
# define _SECP256K1_NODE_ECDSA_

#include <node.h>
#include <nan.h>

NAN_METHOD(sign);
NAN_METHOD(verify);
NAN_METHOD(recover);

#endif
