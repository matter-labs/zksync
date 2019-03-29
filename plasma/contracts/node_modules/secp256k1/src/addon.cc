#include <node.h>
#include <nan.h>
#include <secp256k1.h>

#include "privatekey.h"
#include "publickey.h"
#include "signature.h"
#include "ecdsa.h"
#include "ecdh.h"

secp256k1_context* secp256k1ctx;

NAN_MODULE_INIT(Init) {
  secp256k1ctx = secp256k1_context_create(
    SECP256K1_CONTEXT_SIGN | SECP256K1_CONTEXT_VERIFY);

  // secret key
  Nan::Export(target, "privateKeyVerify", privateKeyVerify);
  Nan::Export(target, "privateKeyExport", privateKeyExport);
  Nan::Export(target, "privateKeyImport", privateKeyImport);
  Nan::Export(target, "privateKeyNegate", privateKeyNegate);
  Nan::Export(target, "privateKeyModInverse", privateKeyModInverse);
  Nan::Export(target, "privateKeyTweakAdd", privateKeyTweakAdd);
  Nan::Export(target, "privateKeyTweakMul", privateKeyTweakMul);

  // public key
  Nan::Export(target, "publicKeyCreate", publicKeyCreate);
  Nan::Export(target, "publicKeyConvert", publicKeyConvert);
  Nan::Export(target, "publicKeyVerify", publicKeyVerify);
  Nan::Export(target, "publicKeyTweakAdd", publicKeyTweakAdd);
  Nan::Export(target, "publicKeyTweakMul", publicKeyTweakMul);
  Nan::Export(target, "publicKeyCombine", publicKeyCombine);

  // signature
  Nan::Export(target, "signatureNormalize", signatureNormalize);
  Nan::Export(target, "signatureExport", signatureExport);
  Nan::Export(target, "signatureImport", signatureImport);
  Nan::Export(target, "signatureImportLax", signatureImportLax);

  // ecdsa
  Nan::Export(target, "sign", sign);
  Nan::Export(target, "verify", verify);
  Nan::Export(target, "recover", recover);

  // ecdh
  Nan::Export(target, "ecdh", ecdh);
  Nan::Export(target, "ecdhUnsafe", ecdhUnsafe);
}

NODE_MODULE(secp256k1, Init)
