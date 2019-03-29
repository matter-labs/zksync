#include <nan.h>
#include <node.h>

#include "scrypt_common.h"

//
// Scrypt is a C library and there needs c linkings
//
extern "C" {
  #include "hash.h"
}

using namespace v8;

//
// Synchronous Scrypt params
//
NAN_METHOD(hashSync) {
  //
  // Arguments from JavaScript
  //
  const uint8_t* key_ptr = reinterpret_cast<uint8_t*>(node::Buffer::Data(info[0]));
  const size_t key_size = node::Buffer::Length(info[0]);
  const NodeScrypt::Params params = info[1]->ToObject();
  const size_t hash_size = info[2]->IntegerValue();
  const uint8_t* salt_ptr = reinterpret_cast<uint8_t*>(node::Buffer::Data(info[3]));
  const size_t salt_size = node::Buffer::Length(info[3]);

  //
  // Variable Declaration
  //
  Local<Value> hash_result = Nan::NewBuffer(static_cast<uint32_t>(hash_size)).ToLocalChecked();
  uint8_t* hash_ptr = reinterpret_cast<uint8_t*>(node::Buffer::Data(hash_result));

  //
  // Scrypt key derivation function
  //
  const unsigned int result = ScryptHashFunction(key_ptr, key_size, salt_ptr, salt_size, params.N, params.r, params.p, hash_ptr, hash_size);

  //
  // Error handling
  //
  if (result) {
    Nan::ThrowError(NodeScrypt::ScryptError(result));
  }

  info.GetReturnValue().Set(hash_result);
}
