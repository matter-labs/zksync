/*
memlimit.h

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
#ifndef _MEMLIMIT_H_
#define _MEMLIMIT_H_

/**
 * memtouse(maxmem, maxmemfrac, memlimit):
 * Examine the system and return via memlimit the amount of RAM which should
 * be used -- the specified fraction of the available RAM, but no more than
 * maxmem, and no less than 1MiB.
 */
int memtouse(size_t, double, size_t, size_t*);

#endif /* !_MEMLIMIT_H_ */
