#include <node.h>
#include <nan.h>
#include <secp256k1.h>
#include <lax_der_parsing.h>

#include "messages.h"
#include "util.h"

extern secp256k1_context* secp256k1ctx;

NAN_METHOD(signatureNormalize) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, ECDSA_SIGNATURE_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(input_buffer, 64, ECDSA_SIGNATURE_LENGTH_INVALID);
  const unsigned char* input = (unsigned char*) node::Buffer::Data(input_buffer);

  secp256k1_ecdsa_signature sigin;
  if (secp256k1_ecdsa_signature_parse_compact(secp256k1ctx, &sigin, input) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_PARSE_FAIL);
  }

  secp256k1_ecdsa_signature sigout;
  secp256k1_ecdsa_signature_normalize(secp256k1ctx, &sigout, &sigin);

  unsigned char output[64];
  secp256k1_ecdsa_signature_serialize_compact(secp256k1ctx, &output[0], &sigout);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], 64));
}

NAN_METHOD(signatureExport) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, ECDSA_SIGNATURE_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(input_buffer, 64, ECDSA_SIGNATURE_LENGTH_INVALID);
  const unsigned char* input = (unsigned char*) node::Buffer::Data(input_buffer);

  secp256k1_ecdsa_signature sig;
  if (secp256k1_ecdsa_signature_parse_compact(secp256k1ctx, &sig, input) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_PARSE_FAIL);
  }

  unsigned char output[72];
  size_t output_length = 72;
  if (secp256k1_ecdsa_signature_serialize_der(secp256k1ctx, &output[0], &output_length, &sig) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_SERIALIZE_DER_FAIL);
  }

  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}

NAN_METHOD(signatureImport) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, ECDSA_SIGNATURE_TYPE_INVALID);
  CHECK_BUFFER_LENGTH_GT_ZERO(input_buffer, ECDSA_SIGNATURE_LENGTH_INVALID);
  const unsigned char* input = (const unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  secp256k1_ecdsa_signature sig;
  if (secp256k1_ecdsa_signature_parse_der(secp256k1ctx, &sig, input, input_length) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_PARSE_DER_FAIL);
  }

  unsigned char output[64];
  secp256k1_ecdsa_signature_serialize_compact(secp256k1ctx, &output[0], &sig);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], 64));
}

NAN_METHOD(signatureImportLax) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> input_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(input_buffer, ECDSA_SIGNATURE_TYPE_INVALID);
  CHECK_BUFFER_LENGTH_GT_ZERO(input_buffer, ECDSA_SIGNATURE_LENGTH_INVALID);
  const unsigned char* input = (const unsigned char*) node::Buffer::Data(input_buffer);
  size_t input_length = node::Buffer::Length(input_buffer);

  secp256k1_ecdsa_signature sig;
  if (ecdsa_signature_parse_der_lax(secp256k1ctx, &sig, input, input_length) == 0) {
    return Nan::ThrowError(ECDSA_SIGNATURE_PARSE_DER_FAIL);
  }

  unsigned char output[64];
  secp256k1_ecdsa_signature_serialize_compact(secp256k1ctx, &output[0], &sig);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], 64));
}
