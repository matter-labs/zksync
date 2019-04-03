/*
  scrypt_node.cc

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
#include <node.h>
#include <nan.h>
#include <v8.h>

using namespace v8;

//
// Forward declarations
//
NAN_METHOD(paramsSync);
NAN_METHOD(params);
NAN_METHOD(kdfSync);
NAN_METHOD(kdf);
NAN_METHOD(kdfVerifySync);
NAN_METHOD(kdfVerify);
NAN_METHOD(hashSync);
NAN_METHOD(hash);

//
// Module initialisation
//
NAN_MODULE_INIT(InitAll) {

  Nan::Set(target, Nan::New<String>("paramsSync").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(paramsSync)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("params").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(params)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("kdfSync").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(kdfSync)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("kdf").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(kdf)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("verifySync").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(kdfVerifySync)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("verify").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(kdfVerify)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("hashSync").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(hashSync)).ToLocalChecked());

  Nan::Set(target, Nan::New<String>("hash").ToLocalChecked(),
    Nan::GetFunction(Nan::New<FunctionTemplate>(hash)).ToLocalChecked());
}

NODE_MODULE(scrypt, InitAll)
