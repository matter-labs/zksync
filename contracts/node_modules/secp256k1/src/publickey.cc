#include <memory>
#include <node.h>
#include <nan.h>
#include <secp256k1.h>

#include "messages.h"
#include "util.h"

extern secp256k1_context* secp256k1ctx;

NAN_METHOD(publicKeyCreate) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> private_key_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  const unsigned char* private_key = (const unsigned char*) node::Buffer::Data(private_key_buffer);

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[1], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_create(secp256k1ctx, &public_key, private_key) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_CREATE_FAIL);
  }

  unsigned char output[65];
  size_t output_length = 65;
  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}

NAN_METHOD(publicKeyConvert) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH2(input_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);
  const unsigned char* input = (unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[1], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, input, input_length) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
  }

  unsigned char output[65];
  size_t output_length = 65;
  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}

NAN_METHOD(publicKeyVerify) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  const unsigned char* input = (unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  secp256k1_pubkey public_key;
  int result = secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, input, input_length);
  info.GetReturnValue().Set(Nan::New<v8::Boolean>(result));
}

NAN_METHOD(publicKeyTweakAdd) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH2(input_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);
  const unsigned char* input = (unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  v8::Local<v8::Object> tweak_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(tweak_buffer, TWEAK_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(tweak_buffer, 32, TWEAK_LENGTH_INVALID);
  const unsigned char* tweak = (const unsigned char *) node::Buffer::Data(tweak_buffer);

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[2], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, input, input_length) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
  }

  if (secp256k1_ec_pubkey_tweak_add(secp256k1ctx, &public_key, tweak) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_TWEAK_ADD_FAIL);
  }

  unsigned char output[65];
  size_t output_length = 65;
  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}

NAN_METHOD(publicKeyTweakMul) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH2(input_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);
  const unsigned char* input = (unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  v8::Local<v8::Object> tweak_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(tweak_buffer, TWEAK_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(tweak_buffer, 32, TWEAK_LENGTH_INVALID);
  const unsigned char* tweak = (const unsigned char *) node::Buffer::Data(tweak_buffer);

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[2], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, input, input_length) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
  }

  if (secp256k1_ec_pubkey_tweak_mul(secp256k1ctx, &public_key, tweak) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_TWEAK_MUL_FAIL);
  }

  unsigned char output[65];
  size_t output_length = 65;
  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}

NAN_METHOD(publicKeyCombine) {
  Nan::HandleScope scope;

  v8::Local<v8::Array> input_buffers = info[0].As<v8::Array>();
  CHECK_TYPE_ARRAY(input_buffers, EC_PUBLIC_KEYS_TYPE_INVALID);
  CHECK_LENGTH_GT_ZERO(input_buffers, EC_PUBLIC_KEYS_LENGTH_INVALID);

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[1], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  std::unique_ptr<secp256k1_pubkey[]> public_keys(new secp256k1_pubkey[input_buffers->Length()]);
  std::unique_ptr<secp256k1_pubkey*[]> ins(new secp256k1_pubkey*[input_buffers->Length()]);
  for (unsigned int i = 0; i < input_buffers->Length(); ++i) {
    v8::Local<v8::Object> public_key_buffer = v8::Local<v8::Object>::Cast(input_buffers->Get(i));
    CHECK_TYPE_BUFFER(public_key_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
    CHECK_BUFFER_LENGTH2(public_key_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);

    const unsigned char* input = (unsigned char*) node::Buffer::Data(public_key_buffer);
    size_t input_length = node::Buffer::Length(public_key_buffer);
    if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_keys[i], input, input_length) == 0) {
      return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
    }

    ins[i] = &public_keys[i];
  }

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_combine(secp256k1ctx, &public_key, ins.get(), input_buffers->Length()) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_COMBINE_FAIL);
  }

  unsigned char output[65];
  size_t output_length = 65;
  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}
