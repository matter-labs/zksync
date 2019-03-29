/*
scrypt_async.h

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

#ifndef _SCRYPTASYNC_H_
#define _SCRYPTASYNC_H_

#include "scrypt_common.h"

//
// Scrypt Async Worker
//

//Note: This class is implemented for common async
// class properties that applies to Scrypt functionality
// only. These properties are:
//  (1) Creation of Scrypt specific Error Object
//  (2) result integer that denotes the response from the Scrypt C library
//  (3) ScryptPeristentObject that holds input arguments from JS land
class ScryptAsyncWorker : public Nan::AsyncWorker {
  public:
    ScryptAsyncWorker(Nan::Callback* callback) : Nan::AsyncWorker(callback), result(0) {};

    //
    // Overrides Nan, needed for creating Scrypt Error
    //
    void HandleErrorCallback() {
      Nan::HandleScope scope;

      v8::Local<v8::Value> argv[] = {
          NodeScrypt::ScryptError(result)
      };
      callback->Call(1, argv);
    }

    //
    // Overrides Nan, needed for checking result
    //
    void WorkComplete() {
      Nan::HandleScope scope;

      if (result == 0)
        HandleOKCallback();
      else
        HandleErrorCallback();

      delete callback;
      callback = NULL;
    }

  protected:
    //
    // Scrypt specific state
    //
    v8::Local<v8::Object> ScryptPeristentObject; // Anything persistent stored here
    unsigned int result; // Result of Scrypt functions
};

#endif /* _SCRYPTASYNC_H_ */
