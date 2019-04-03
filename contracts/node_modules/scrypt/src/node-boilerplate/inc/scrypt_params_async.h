/*
scrypt_params_async.h

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

#ifndef _SCRYPT_PARAMS_ASYNC_H
#define _SCRYPT_PARAMS_ASYNC_H

#include "scrypt_async.h"

// Async class
class ScryptParamsAsyncWorker : public ScryptAsyncWorker {
  public:
    ScryptParamsAsyncWorker(Nan::NAN_METHOD_ARGS_TYPE info) :
      ScryptAsyncWorker(new Nan::Callback(info[4].As<v8::Function>())),
      maxtime(info[0]->NumberValue()),
      maxmemfrac(info[1]->NumberValue()),
      maxmem(info[2]->IntegerValue()),
      osfreemem(info[3]->IntegerValue())
    {
      logN = 0;
      r = 0;
      p = 0;
    };

    void Execute();
    void HandleOKCallback();

  private:
    const double maxtime;
    const double maxmemfrac;
    const size_t maxmem;
    const size_t osfreemem;

    int logN;
    uint32_t r;
    uint32_t p;
};

#endif /* _SCRYPT_PARAMS_ASYNC_H */
