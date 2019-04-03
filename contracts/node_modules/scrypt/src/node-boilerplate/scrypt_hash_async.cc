/*
scrypt_hash_async.cc

Copyright (C) 2013 Barry Steyn (http://doctrina.org/Scrypt-Authentication-For-Node.html)

This source code is provided 'as-is', without any express or implied
warranty. In no event will the author be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this source code must not be misrepresented; you must not
   claim that you wrote the original source code. If you use this source code
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original source code.
3. This notice may not be removed or altered from any source distribution.

Barry Steyn barry.steyn@gmail.com
*/

#include <nan.h>
#include <node.h>

#include "scrypt_hash_async.h"

//C linkings needed for Scrypt
extern "C" {
  #include "hash.h"
}

using namespace v8;

//
// Scrypt Hash Function
//
void ScryptHashAsyncWorker::Execute() {
  result = ScryptHashFunction(key_ptr, key_size, salt_ptr, salt_size, params.N, params.r, params.p, hash_ptr, hash_size);
}

void ScryptHashAsyncWorker::HandleOKCallback() {
  Nan::HandleScope scope;

  Local<Value> argv[] = {
    Nan::Null(),
    GetFromPersistent("ScryptPeristentObject")->ToObject()->Get(Nan::New("HashBuffer").ToLocalChecked())
  };

  callback->Call(2, argv);
}

//
// Asynchronous Scrypt Params
//
NAN_METHOD(hash) {
  Nan::AsyncQueueWorker(new ScryptHashAsyncWorker(info));
}
