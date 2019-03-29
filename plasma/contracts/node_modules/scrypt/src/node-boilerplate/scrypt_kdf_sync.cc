#include <nan.h>
#include <node.h>

#include "scrypt_common.h"

//
// Scrypt is a C library and there needs c linkings
//
extern "C" {
	#include "keyderivation.h"
}

using namespace v8;

//
// Synchronous Scrypt params
//
NAN_METHOD(kdfSync) {
    //
    // Variable Declaration
    //
    Local<Value> kdfResult = Nan::NewBuffer(96).ToLocalChecked();

    //
    // Arguments from JavaScript
    //
    const uint8_t* key_ptr = reinterpret_cast<uint8_t*>(node::Buffer::Data(info[0])); //assume info[0] is a buffer (checked in JS land)
    const size_t keySize = node::Buffer::Length(info[0]);
    const NodeScrypt::Params params = info[1]->ToObject();
    const uint8_t* salt_ptr = reinterpret_cast<uint8_t*>(node::Buffer::Data(info[2]));

    //
    // Scrypt key derivation function
    //
    const unsigned int result = KDF(key_ptr, keySize, reinterpret_cast<uint8_t*>(node::Buffer::Data(kdfResult)), params.N, params.r, params.p, salt_ptr);

    //
    // Error handling
    //
    if (result) {
        Nan::ThrowError(NodeScrypt::ScryptError(result));
    }

    info.GetReturnValue().Set(kdfResult);
}
