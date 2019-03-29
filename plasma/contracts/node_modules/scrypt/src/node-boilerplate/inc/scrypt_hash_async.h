/*
scrypt_hash_async.h

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

#ifndef _SCRYPTHASHASYNC_
#define _SCRYPTHASHASYNC_

#include "scrypt_async.h"

class ScryptHashAsyncWorker : public ScryptAsyncWorker {
  public:
    ScryptHashAsyncWorker(Nan::NAN_METHOD_ARGS_TYPE info) :
      ScryptAsyncWorker(new Nan::Callback(info[4].As<v8::Function>())),
      key_ptr(reinterpret_cast<uint8_t*>(node::Buffer::Data(info[0]))),
      key_size(node::Buffer::Length(info[0])),
      params(info[1]->ToObject()),
      hash_size(info[2]->IntegerValue()),
      salt_ptr(reinterpret_cast<uint8_t*>(node::Buffer::Data(info[3]))),
      salt_size(static_cast<size_t>(node::Buffer::Length(info[3])))
    {
      ScryptPeristentObject = Nan::New<v8::Object>();
      ScryptPeristentObject->Set(Nan::New("KeyBuffer").ToLocalChecked(), info[0]);
      ScryptPeristentObject->Set(Nan::New("HashBuffer").ToLocalChecked(), Nan::NewBuffer(static_cast<uint32_t>(hash_size)).ToLocalChecked());
      ScryptPeristentObject->Set(Nan::New("SaltBuffer").ToLocalChecked(), info[3]);
      SaveToPersistent("ScryptPeristentObject", ScryptPeristentObject);

      hash_ptr = reinterpret_cast<uint8_t*>(node::Buffer::Data(ScryptPeristentObject->Get(Nan::New("HashBuffer").ToLocalChecked())));
    };

    void Execute();
    void HandleOKCallback();

  private:
    const uint8_t* key_ptr;
    const size_t key_size;
    const NodeScrypt::Params params;
    const size_t hash_size;
    const uint8_t* salt_ptr;
    const size_t salt_size;
    uint8_t* hash_ptr;
};

#endif /* _SCRYPTHASHASYNC_ */
