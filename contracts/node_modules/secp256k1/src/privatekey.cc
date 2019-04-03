#include <node.h>
#include <nan.h>
#include <secp256k1.h>
#include <scalar_impl.h>
#include <lax_der_privatekey_parsing.h>

#include "messages.h"
#include "util.h"

extern secp256k1_context* secp256k1ctx;

NAN_METHOD(privateKeyVerify) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  const unsigned char* private_key = (const unsigned char*) node::Buffer::Data(private_key_buffer);

  if (node::Buffer::Length(private_key_buffer) != 32) {
    return info.GetReturnValue().Set(Nan::New<v8::Boolean>(false));
  }

  int result = secp256k1_ec_seckey_verify(secp256k1ctx, private_key);
  info.GetReturnValue().Set(Nan::New<v8::Boolean>(result));
}

NAN_METHOD(privateKeyExport) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  const unsigned char* private_key = (const unsigned char*) node::Buffer::Data(private_key_buffer);

  int compressed = 1;
  UPDATE_COMPRESSED_VALUE(compressed, info[1], 1, 0);

  unsigned char output[279];
  size_t output_length;
  if (ec_privkey_export_der(secp256k1ctx, &output[0], &output_length, private_key, compressed) == 0) {
    return Nan::ThrowError(EC_PRIVATE_KEY_EXPORT_DER_FAIL);
  }

  info.GetReturnValue().Set(COPY_BUFFER(output, output_length));
}

NAN_METHOD(privateKeyImport) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH_GT_ZERO(input_buffer, EC_PRIVATE_KEY_LENGTH_INVALID);
  const unsigned char* input = (const unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  unsigned char private_key[32];
  if (ec_privkey_import_der(secp256k1ctx, &private_key[0], input, input_length) == 0) {
    return Nan::ThrowError(EC_PRIVATE_KEY_IMPORT_DER_FAIL);
  }

  info.GetReturnValue().Set(COPY_BUFFER(private_key, 32));
}

NAN_METHOD(privateKeyNegate) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  unsigned char private_key[32];
  memcpy(&private_key[0], node::Buffer::Data(private_key_buffer), 32);

  secp256k1_ec_privkey_negate(secp256k1ctx, &private_key[0]);

  info.GetReturnValue().Set(COPY_BUFFER(&private_key[0], 32));
}

NAN_METHOD(privateKeyModInverse) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  unsigned char private_key[32];
  memcpy(&private_key[0], node::Buffer::Data(private_key_buffer), 32);

  secp256k1_scalar s;
  int overflow = 0;
  secp256k1_scalar_set_b32(&s, private_key, &overflow);
  if (overflow || secp256k1_scalar_is_zero(&s)) {
    secp256k1_scalar_clear(&s);
    return Nan::ThrowError(EC_PRIVATE_KEY_RANGE_INVALID);
  }

  secp256k1_scalar_inverse(&s, &s);

  secp256k1_scalar_get_b32(private_key, &s);
  secp256k1_scalar_clear(&s);

  info.GetReturnValue().Set(COPY_BUFFER(&private_key[0], 32));
}

NAN_METHOD(privateKeyTweakAdd) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  unsigned char private_key[32];
  memcpy(&private_key[0], node::Buffer::Data(private_key_buffer), 32);

  v8::Local<v8::Object> tweak_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(tweak_buffer, TWEAK_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(tweak_buffer, 32, TWEAK_LENGTH_INVALID);
  const unsigned char* tweak = (unsigned char *) node::Buffer::Data(tweak_buffer);

  if (secp256k1_ec_privkey_tweak_add(secp256k1ctx, &private_key[0], tweak) == 0) {
    return Nan::ThrowError(EC_PRIVATE_KEY_TWEAK_ADD_FAIL);
  }

  info.GetReturnValue().Set(COPY_BUFFER(&private_key[0], 32));
}

NAN_METHOD(privateKeyTweakMul) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  unsigned char private_key[32];
  memcpy(&private_key[0], node::Buffer::Data(private_key_buffer), 32);

  v8::Local<v8::Object> tweak_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(tweak_buffer, TWEAK_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(tweak_buffer, 32, TWEAK_LENGTH_INVALID);
  const unsigned char* tweak = (unsigned char *) node::Buffer::Data(tweak_buffer);

  if (secp256k1_ec_privkey_tweak_mul(secp256k1ctx, &private_key[0], tweak) == 0) {
    return Nan::ThrowError(EC_PRIVATE_KEY_TWEAK_MUL_FAIL);
  }

  info.GetReturnValue().Set(COPY_BUFFER(&private_key[0], 32));
}
