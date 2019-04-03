#include <node.h>
#include <nan.h>
#include <secp256k1.h>
#include <util.h>
#include <field_impl.h>
#include <scalar_impl.h>
#include <group_impl.h>
#include <ecmult_const_impl.h>
#include <ecmult_gen_impl.h>

#include "messages.h"
#include "util.h"

extern secp256k1_context* secp256k1ctx;

// from bitcoin/secp256k1
#define ARG_CHECK(cond) do { \
    if (EXPECT(!(cond), 0)) { \
        secp256k1_callback_call(&ctx->illegal_callback, #cond); \
        return 0; \
    } \
} while(0)

static void default_illegal_callback_fn(const char* str, void* data) {
    (void)data;
    fprintf(stderr, "[libsecp256k1] illegal argument: %s\n", str);
    abort();
}

static const secp256k1_callback default_illegal_callback = {
    default_illegal_callback_fn,
    NULL
};

static void default_error_callback_fn(const char* str, void* data) {
    (void)data;
    fprintf(stderr, "[libsecp256k1] internal consistency check failed: %s\n", str);
    abort();
}

static const secp256k1_callback default_error_callback = {
    default_error_callback_fn,
    NULL
};

struct secp256k1_context_struct {
    secp256k1_ecmult_context ecmult_ctx;
    secp256k1_ecmult_gen_context ecmult_gen_ctx;
    secp256k1_callback illegal_callback;
    secp256k1_callback error_callback;
};

int secp256k1_pubkey_load(const secp256k1_context* ctx, secp256k1_ge* ge, const secp256k1_pubkey* pubkey) {
    if (sizeof(secp256k1_ge_storage) == 64) {
        /* When the secp256k1_ge_storage type is exactly 64 byte, use its
         * representation inside secp256k1_pubkey, as conversion is very fast.
         * Note that secp256k1_pubkey_save must use the same representation. */
        secp256k1_ge_storage s;
        memcpy(&s, &pubkey->data[0], 64);
        secp256k1_ge_from_storage(ge, &s);
    } else {
        /* Otherwise, fall back to 32-byte big endian for X and Y. */
        secp256k1_fe x, y;
        secp256k1_fe_set_b32(&x, pubkey->data);
        secp256k1_fe_set_b32(&y, pubkey->data + 32);
        secp256k1_ge_set_xy(ge, &x, &y);
    }
    ARG_CHECK(!secp256k1_fe_is_zero(&ge->x));
    return 1;
}

void secp256k1_pubkey_save(secp256k1_pubkey* pubkey, secp256k1_ge* ge) {
    if (sizeof(secp256k1_ge_storage) == 64) {
        secp256k1_ge_storage s;
        secp256k1_ge_to_storage(&s, ge);
        memcpy(&pubkey->data[0], &s, 64);
    } else {
        VERIFY_CHECK(!secp256k1_ge_is_infinity(ge));
        secp256k1_fe_normalize_var(&ge->x);
        secp256k1_fe_normalize_var(&ge->y);
        secp256k1_fe_get_b32(pubkey->data, &ge->x);
        secp256k1_fe_get_b32(pubkey->data + 32, &ge->y);
    }
}

// bindings
NAN_METHOD(ecdh) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> pubkey_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(pubkey_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH2(pubkey_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);
  const unsigned char* public_key_input = (unsigned char*) node::Buffer::Data(pubkey_buffer);
  size_t public_key_input_length = node::Buffer::Length(pubkey_buffer);

  v8::Local<v8::Object> private_key_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  const unsigned char* private_key = (const unsigned char*) node::Buffer::Data(private_key_buffer);

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, public_key_input, public_key_input_length) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
  }

  secp256k1_scalar s;
  int overflow = 0;
  secp256k1_scalar_set_b32(&s, private_key, &overflow);
  if (overflow || secp256k1_scalar_is_zero(&s)) {
    secp256k1_scalar_clear(&s);
    return Nan::ThrowError(ECDH_FAIL);
  }

  secp256k1_ge pt;
  secp256k1_gej res;
  unsigned char y[1];
  unsigned char x[32];
  secp256k1_sha256_t sha;
  unsigned char output[32];

  secp256k1_pubkey_load(secp256k1ctx, &pt, &public_key);
  secp256k1_ecmult_const(&res, &pt, &s);
  secp256k1_scalar_clear(&s);

  secp256k1_ge_set_gej(&pt, &res);
  secp256k1_fe_normalize(&pt.y);
  secp256k1_fe_normalize(&pt.x);

  y[0] = 0x02 | secp256k1_fe_is_odd(&pt.y);
  secp256k1_fe_get_b32(&x[0], &pt.x);

  secp256k1_sha256_initialize(&sha);
  secp256k1_sha256_write(&sha, y, sizeof(y));
  secp256k1_sha256_write(&sha, x, sizeof(x));
  secp256k1_sha256_finalize(&sha, &output[0]);

  info.GetReturnValue().Set(COPY_BUFFER(&output[0], 32));
}

NAN_METHOD(ecdhUnsafe) {
  Nan::HandleScope scope;

  v8::Local<v8::Object> pubkey_buffer = info[0].As<v8::Object>();
  CHECK_TYPE_BUFFER(pubkey_buffer, EC_PUBLIC_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH2(pubkey_buffer, 33, 65, EC_PUBLIC_KEY_LENGTH_INVALID);
  const unsigned char* public_key_input = (unsigned char*) node::Buffer::Data(pubkey_buffer);
  size_t public_key_input_length = node::Buffer::Length(pubkey_buffer);

  v8::Local<v8::Object> private_key_buffer = info[1].As<v8::Object>();
  CHECK_TYPE_BUFFER(private_key_buffer, EC_PRIVATE_KEY_TYPE_INVALID);
  CHECK_BUFFER_LENGTH(private_key_buffer, 32, EC_PRIVATE_KEY_LENGTH_INVALID);
  const unsigned char* private_key = (const unsigned char*) node::Buffer::Data(private_key_buffer);

  secp256k1_pubkey public_key;
  if (secp256k1_ec_pubkey_parse(secp256k1ctx, &public_key, public_key_input, public_key_input_length) == 0) {
    return Nan::ThrowError(EC_PUBLIC_KEY_PARSE_FAIL);
  }

  unsigned int flags = SECP256K1_EC_COMPRESSED;
  UPDATE_COMPRESSED_VALUE(flags, info[2], SECP256K1_EC_COMPRESSED, SECP256K1_EC_UNCOMPRESSED);

  secp256k1_scalar s;
  int overflow = 0;
  secp256k1_scalar_set_b32(&s, private_key, &overflow);
  if (overflow || secp256k1_scalar_is_zero(&s)) {
    secp256k1_scalar_clear(&s);
    return Nan::ThrowError(ECDH_FAIL);
  }

  secp256k1_ge pt;
  secp256k1_gej res;
  unsigned char output[65];
  size_t output_length = 65;

  secp256k1_pubkey_load(secp256k1ctx, &pt, &public_key);
  secp256k1_ecmult_const(&res, &pt, &s);
  secp256k1_scalar_clear(&s);

  secp256k1_ge_set_gej(&pt, &res);
  secp256k1_pubkey_save(&public_key, &pt);

  secp256k1_ec_pubkey_serialize(secp256k1ctx, &output[0], &output_length, &public_key, flags);
  info.GetReturnValue().Set(COPY_BUFFER(&output[0], output_length));
}
