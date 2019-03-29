#ifndef _SECP256K1_NODE_PRIVATEKEY_
# define _SECP256K1_NODE_PRIVATEKEY_

#include <node.h>
#include <nan.h>

NAN_METHOD(privateKeyVerify);
NAN_METHOD(privateKeyExport);
NAN_METHOD(privateKeyImport);
NAN_METHOD(privateKeyNegate);
NAN_METHOD(privateKeyModInverse);
NAN_METHOD(privateKeyTweakAdd);
NAN_METHOD(privateKeyTweakMul);

#endif
