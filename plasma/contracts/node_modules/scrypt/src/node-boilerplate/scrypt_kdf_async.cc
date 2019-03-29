#include <nan.h>
#include <node.h>

#include "scrypt_kdf_async.h"

//Scrypt is a C library and there needs c linkings
extern "C" {
	#include "keyderivation.h"
}

using namespace v8;

void ScryptKDFAsyncWorker::Execute() {
    //
    // Scrypt key derivation function
    //
    result = KDF(key_ptr, key_size, KDFResult_ptr, params.N, params.r, params.p, salt_ptr);
}

void ScryptKDFAsyncWorker::HandleOKCallback() {
    Nan::HandleScope scope;

    Local<Value> argv[] = {
        Nan::Null(),
        GetFromPersistent("ScryptPeristentObject")->ToObject()->Get(Nan::New("KDFResult").ToLocalChecked())
    };

    callback->Call(2, argv);
}

// Asynchronous access to scrypt params
NAN_METHOD(kdf) {
    Nan::AsyncQueueWorker(new ScryptKDFAsyncWorker(info));
}
