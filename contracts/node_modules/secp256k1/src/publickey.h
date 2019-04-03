#ifndef _SECP256K1_NODE_PUBLICKEY_
# define _SECP256K1_NODE_PUBLICKEY_

#include <node.h>
#include <nan.h>

NAN_METHOD(publicKeyCreate);
NAN_METHOD(publicKeyConvert);
NAN_METHOD(publicKeyVerify);
NAN_METHOD(publicKeyTweakAdd);
NAN_METHOD(publicKeyTweakMul);
NAN_METHOD(publicKeyCombine);

#endif
