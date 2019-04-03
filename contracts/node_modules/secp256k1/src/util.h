#ifndef _SECP256K1_NODE_UTIL_
# define _SECP256K1_NODE_UTIL_

#include <node.h>
#include <nan.h>

#include "messages.h"

#define COPY_BUFFER(data, datalen) Nan::CopyBuffer((const char*) data, (uint32_t) datalen).ToLocalChecked()

#define UPDATE_COMPRESSED_VALUE(compressed, value, v_true, v_false) {          \
  if (!value->IsUndefined()) {                                                 \
    CHECK_TYPE_BOOLEAN(value, COMPRESSED_TYPE_INVALID);                        \
    compressed = value->BooleanValue() ? v_true : v_false;                     \
  }                                                                            \
}

// TypeError
#define CHECK_TYPE_ARRAY(value, message) {                                     \
  if (!value->IsArray()) {                                                     \
    return Nan::ThrowTypeError(message);                                       \
  }                                                                            \
}

#define CHECK_TYPE_BOOLEAN(value, message) {                                   \
  if (!value->IsBoolean() && !value->IsBooleanObject()) {                      \
    return Nan::ThrowTypeError(message);                                       \
  }                                                                            \
}

#define CHECK_TYPE_BUFFER(value, message) {                                    \
  if (!node::Buffer::HasInstance(value)) {                                     \
    return Nan::ThrowTypeError(message);                                       \
  }                                                                            \
}

#define CHECK_TYPE_FUNCTION(value, message) {                                  \
  if (!value->IsFunction()) {                                                  \
    return Nan::ThrowTypeError(message);                                       \
  }                                                                            \
}

#define CHECK_TYPE_NUMBER(value, message) {                                    \
  if (!value->IsNumber() && !value->IsNumberObject()) {                        \
    return Nan::ThrowTypeError(message);                                       \
  }                                                                            \
}

#define CHECK_TYPE_OBJECT(value, message) {                                    \
  if (!value->IsObject()) {                                                    \
    return Nan::ThrowTypeError(message);                                       \
  }                                                                            \
}

// RangeError
#define CHECK_BUFFER_LENGTH(buffer, length, message) {                         \
  if (node::Buffer::Length(buffer) != length) {                                \
    return Nan::ThrowRangeError(message);                                      \
  }                                                                            \
}

#define CHECK_BUFFER_LENGTH2(buffer, length1, length2, message) {              \
  if (node::Buffer::Length(buffer) != length1 &&                               \
      node::Buffer::Length(buffer) != length2) {                               \
    return Nan::ThrowRangeError(message);                                      \
  }                                                                            \
}

#define CHECK_BUFFER_LENGTH_GT_ZERO(buffer, message) {                         \
  if (node::Buffer::Length(buffer) == 0) {                                     \
    return Nan::ThrowRangeError(message);                                      \
  }                                                                            \
}

#define CHECK_LENGTH_GT_ZERO(value, message) {                                 \
  if (value->Length() == 0) {                                                  \
    return Nan::ThrowRangeError(message);                                      \
  }                                                                            \
}

#define CHECK_NUMBER_IN_INTERVAL(number, x, y, message) {                      \
  if (number->IntegerValue() <= x || number->IntegerValue() >= y) {            \
    return Nan::ThrowRangeError(message);                                      \
  }                                                                            \
}

#endif
