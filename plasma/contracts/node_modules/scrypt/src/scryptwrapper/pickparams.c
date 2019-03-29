/*
pickparams.c

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

#include <stdint.h>
#include <sys/types.h>

#include "pickparams.h"
#include "scryptenc_cpuperf.h"
#include "util/memlimit.h"


///remove

#include <stdio.h>
//end remove

/*
 * Given maxmem, maxmemfrac and maxtime, this functions calculates the N,r,p variables.
 * Values for N,r,p are machine dependent. This is copied directly from Colin Percival's srypt reference code
 */
unsigned int
pickparams(int *logN, uint32_t *r, uint32_t *p, double maxtime, size_t maxmem, double maxmemfrac, size_t osfreemem) {
    //Note: logN (as opposed to N) is calculated here. This is because it is compact (it can be represented by an int)
    //      and it is easy (and quick) to convert to N by right shifting bits. Most importantly, using logN only requires
    //      32 bits to be stored. Seeing as it is embedded inside the hash, the smaller the better
    size_t memlimit;
    double opps;
    double opslimit;
    double maxN, maxrp;
    int rc;

    /* Figure out how much memory to use. */
    if (memtouse(maxmem, maxmemfrac, osfreemem, &memlimit))
        return (1);

    /* Figure out how fast the CPU is. */
    if ((rc = scryptenc_cpuperf(&opps)) != 0)
        return ((unsigned int)(rc)); // type cast works since Colin is only using positive integers
    opslimit = opps * maxtime;

    /* Allow a minimum of 2^15 salsa20/8 cores. */
    if (opslimit < 32768)
        opslimit = 32768;

    /* Set r to 8 */
    *r = 8; // r is the underlying block size, Colin Percival defaults to 8 in his reference implementation

    /*
    * The memory limit requires that 128Nr <= memlimit, while the CPU
    * limit requires that 4Nrp <= opslimit. If opslimit < memlimit/32,
    * opslimit imposes the stronger limit on N.
    */
    if (opslimit < memlimit/32) {
        /* Set p = 1 and choose N based on the CPU limit. */
        *p = 1;
        maxN = opslimit / (*r * 4);
        for (*logN = 1; *logN < 63; *logN += 1) {
            if ((uint64_t)(1) << *logN > maxN / 2)
                break;
        }
    } else {
        /* Set N based on the memory limit. */
        maxN = (double)(memlimit / (*r * 128));
        for (*logN = 1; *logN < 63; *logN += 1) {
            if ((uint64_t)(1) << *logN > maxN / 2)
            break;
        }

        /* Choose p based on the CPU limit. */
        maxrp = (opslimit / 4) / ((uint64_t)(1) << *logN);
        if (maxrp > 0x3fffffff)
            maxrp = 0x3fffffff;
        *p = (uint32_t)(maxrp) / *r;
    }

    /* Success! */
    return (0);
}
