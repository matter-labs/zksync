/*
hash.c

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

#include <stdio.h>

#include <sys/types.h>
#include <errno.h>
#include "crypto_scrypt.h"
#include "pickparams.h"

//
// This is the actual key derivation function.
// It is binary safe and is exposed to this module for
// access to the underlying key derivation function of Scrypt
//
unsigned int
ScryptHashFunction(const uint8_t* key, size_t keylen, const uint8_t *salt, size_t saltlen, uint64_t N, uint32_t r, uint32_t p,uint8_t *buf, size_t buflen) {
  int rc = crypto_scrypt(key, keylen, salt, saltlen, N, r, p, buf, buflen);
  unsigned int error = (rc == 0) ? 0 : 3;

  if (error && errno) {
    error |= (errno << 16);
  }

  return (error);
}
