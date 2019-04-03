#include <node.h>
#include <nan.h>
#include <secp256k1.h>
#include <secp256k1_recovery.h>

#include "messages.h"
#include "util.h"

extern secp256k1_context* secp256k1ctx;

v8::Local<v8::Function> noncefn_callback;
int nonce_function_custom(unsigned char *nonce32, const unsigned char *msg32, const unsigned char *key32, const unsigned char *algo16, void *data, unsigned int counter) {
  v8::Local<v8::Value> argv[] = {
    COPY_BUFFER(msg32, 32),
    COPY_BUFFER(key32, 32),
    algo16 == NULL ? v8::Local<v8::Value>(Nan::Null()) : v8::Local<v8::Value>(COPY_BUFFER(algo16, 16)),
    data == NULL ? v8::Local<v8::Value>(Nan::Null()) : v8::Local<v8::Value>(COPY_BUFFER(data, 32)),
    Nan::New(counter)
  };

#if (NODE_MODULE_VERSION > NODE_0_10_MODULE_VERSION)
  v8::Isolate *isolate = v8::Isolate::GetCurrent();
  v8::Local<v8::Value> result = noncefn_callback->Call(isolate->GetCurrentContext()->Global(), 5, argv);
#else
  v8::Local<v8::Value> result = noncefn_callback->Call(v8::Context::GetCurrent()->Global(), 5, argv);
#endif

  if (!node::Buffer::HasInstance(result) || node::Buffer::Length(result) != 32) {
    return 0;
  }

  memcpy(nonce32, node::Buffer::Data(result), 32);
  return 1;
}

NAN_METHOD(sign) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> msg32_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(msg32_buffer, MSG32_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(msg32_buffer, 32, MSG32_LENGTH_INVALID);
  const unsigned char* msg32 = (const unsigned char*) node::Buffer::Data(msg32_buffer);

  v8::Local<v8::Object> private_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  const unsigned char* private_key = (const unsigned char*) node::Buffer::Data(private_buffer);

  secp256k1_nonce_function noncefn = secp256k1_nonce_function_rfc6979;
  void* data = NULL;
  v8::Local<v8::Object> options = info[2].As<v8::Object>();
  if (!options->IsUndefined()) {
    CHECK_TYPE_OBJECT(options, OPTIONS_TYPE_INVALID);

    v8::Local<v8::Value> data_value = options->Get(Nan::New<v8::String>("data").ToLocalChecked());
    if (!data_value->IsUndefined()) {
      CHECK_TYPE_BUFFER(data_value, OPTIONS_DATA_TYPE_INVALID);
      CHECK_BUFFER_LENGTH(data_value, 32, OPTIONS_DATA_LENGTH_INVALID);
      data = (void*) node::Buffer::Data(data_value);
    }

    noncefn_callback = v8::Local<v8::Function>::Cast(options->Get(Nan::New<v8::String>("noncefn").ToLocalChecked()));
    if (!noncefn_callback->IsUndefined()) {
      CHECK_TYPE_FUNCTION(noncefn_callback, OPTIONS_NONCEFN_TYPE_INVALID);
      noncefn = nonce_function_custom;
    }
  }

  secp256k1_ecdsa_recoverable_signature sig;
  if (secp256k1_ecdsa_sign_recoverable(secp256k1ctx, &sig, msg32, private_key, noncefn, data) == 0) {
    return Nan::ThrowError(ECDSA_SIGN_FAIL);
  }

  unsigned char output[64];
  int recid;
  secp256k1_ecdsa_recoverable_signature_serialize_compact(secp256k1ctx, &output[0], &recid, &sig);

  v8::Local<v8::Object> obj = Nan::New<v8::Object>();
  obj->Set(Nan::New<v8::String>("signature").ToLocalChecked(), COPY_BUFFER(&output[0], 64));
  obj->Set(Nan::New<v8::String>("recovery").ToLocalChecked(), Nan::New<v8::Number>(recid));
  info.GetReturnValue().Set(obj);
}

NAN_METHOD(verify) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> msg32_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(msg32_buffer, MSG32_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(msg32_buffer, 32, MSG32_LENGTH_INVALID);
  const unsigned char* msg32 = (const unsigned char*) node::Buffer::Data(msg32_buffer);

  v8::Local<v8::Object> sig_input_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(sig_input_buffer, ECDSA_SIGNATURE_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(sig_input_buffer, 64, ECDSA_SIGNATURE_LENGTH_INVALID);
  const unsigned char* sig_input = (unsigned char*) node::Buffer::Data(sig_input_buffer);

  v8::Local<v8::Object> public_key_buffer = info[2].As<v8::Object>();
  CHECK_TYPE_BUFFER(public_key_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH2(public_key_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);
  const unsigned char* public_key_input = (unsigned char*) node::Buffer::Data(public_key_buffer);
  size_t public_key_input_length = node::Buffer::Length(public_key_buffer);

  secp256k1_ecdsa_signature sig;
  if (secp256k1_ecdsa_signature_parse_compact(secp256k1ctx, &sig, sig_input) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_PARSE_FAIL);
  }

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, public_key_input, public_key_input_length) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
  }

  int result = secp256k1_ecdsa_verify(secp256k1ctx, &sig, msg32, &public_key);
  info.GetReturnValue().Set(Nan::New<v8::Boolean>(result));
}

NAN_METHOD(recover) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> msg32_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(msg32_buffer, MSG32_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(msg32_buffer, 32, MSG32_LENGTH_INVALID);
  const unsigned char* msg32 = (const unsigned char*) node::Buffer::Data(msg32_buffer);

  v8::Local<v8::Object> sig_input_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(sig_input_buffer, ECDSA_SIGNATURE_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(sig_input_buffer, 64, ECDSA_SIGNATURE_LENGTH_INVALID);
  const unsigned char* sig_input = (unsigned char*) node::Buffer::Data(sig_input_buffer);

  v8::Local<v8::Object> recid_object = info[2].As<v8::Object>();
  CHECK_TYPE_NUMBER(recid_object, RECOVERY_ID_TYPE_INVALID);
  CHECK_NUMBER_IN_INTERVAL(recid_object, -1, 4, RECOVERY_ID_VALUE_INVALID);
  int recid = (int) recid_object->IntegerValue();

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[3], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  secp256k1_ecdsa_recoverable_signature sig;
  if (secp256k1_ecdsa_recoverable_signature_parse_compact(secp256k1ctx, &sig, sig_input, recid) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_PARSE_FAIL);
  }

  secp256k1_pubkey public_key;
  if (secp256k1_ecdsa_recover(secp256k1ctx, &public_key, &sig, msg32) == 0) {
    return Nan::ThrowError(ECDSA_RECOVER_FAIL);
  }

  unsigned char output[65];
  size_t output_length = 65;
  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}
