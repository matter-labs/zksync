#include <nan.h>
#include <node.h>

#include "scrypt_params_async.h"

//Scrypt is a C library and there needs c linkings
extern "C" {
  #include "pickparams.h"
}

using namespace v8;

void ScryptParamsAsyncWorker::Execute() {
  // Scrypt: calculate input parameters
  result = pickparams(&logN, &r, &p, maxtime, maxmem, maxmemfrac, osfreemem);
}

void ScryptParamsAsyncWorker::HandleOKCallback() {
  Nan::HandleScope scope;

  // Returned params in JSON object
  Local <Object> obj = Nan::New<Object>();
  obj->Set(Nan::New("N").ToLocalChecked(), Nan::New<Integer>(logN));
  obj->Set(Nan::New("r").ToLocalChecked(), Nan::New<Integer>(r));
  obj->Set(Nan::New("p").ToLocalChecked(), Nan::New<Integer>(p));

  Local<Value> argv[] = {
    Nan::Null(),
    obj
  };

  callback->Call(2, argv);
}

// Asynchronous access to scrypt params
NAN_METHOD(params) {
  Nan::AsyncQueueWorker(new ScryptParamsAsyncWorker(info));
}
