#include <nan.h>
#include <node.h>

#include "scrypt_common.h"

//Scrypt is a C library and there needs c linkings
extern "C" {
  #include "pickparams.h"
}

using namespace v8;

//Synchronous access to scrypt params
NAN_METHOD(paramsSync) {
  //
  // Variable Declaration
  //
  int logN = 0;
  uint32_t r = 0;
  uint32_t p = 0;

  //
  // Arguments from JavaScript
  //
  const double maxtime = info[0]->NumberValue();
  const size_t maxmem = info[2]->IntegerValue();
  const double maxmemfrac = info[1]->NumberValue();
  const size_t osfreemem = info[3]->IntegerValue();

  //
  // Scrypt: calculate input parameters
  //
  const unsigned int result = pickparams(&logN, &r, &p, maxtime, maxmem, maxmemfrac, osfreemem);

  //
  // Error handling
  //
  if (result) {
    Nan::ThrowError(NodeScrypt::ScryptError(result));
  }

  //
  // Return values in JSON object
  //
  Local <Object> obj = Nan::New<Object>();
  obj->Set(Nan::New("N").ToLocalChecked(), Nan::New<Integer>(logN));
  obj->Set(Nan::New("r").ToLocalChecked(), Nan::New<Integer>(r));
  obj->Set(Nan::New("p").ToLocalChecked(), Nan::New<Integer>(p));

  info.GetReturnValue().Set(obj);
}
