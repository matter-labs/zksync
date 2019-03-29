/*-
 * Copyright 2009 Colin Percival
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 *
 * This file was originally written by Colin Percival as part of the Tarsnap
 * online backup system.
 */
#include "scrypt_platform.h"

#ifndef _MSC_VER
#include <sys/time.h>
#else /* For 'struct timeval' and custom gettimeofday() on windows */
#include <winsock2.h>
#include "gettimeofday.h"
#endif /* _MSC_VER */

#include <stdint.h>
#include <stdio.h>
#include <time.h>

#include "crypto_scrypt.h"

#include "scryptenc_cpuperf.h"

#ifdef HAVE_CLOCK_GETTIME

static clock_t clocktouse;

static int
getclockres(double * resd)
{
	struct timespec res;

	/*
	 * Try clocks in order of preference until we find one which works.
	 * (We assume that if clock_getres works, clock_gettime will, too.)
	 * The use of if/else/if/else/if/else rather than if/elif/elif/else
	 * is ugly but legal, and allows us to #ifdef things appropriately.
	 */
#ifdef CLOCK_VIRTUAL
	if (clock_getres(CLOCK_VIRTUAL, &res) == 0)
		clocktouse = CLOCK_VIRTUAL;
	else
#endif
#ifdef CLOCK_MONOTONIC
	if (clock_getres(CLOCK_MONOTONIC, &res) == 0)
		clocktouse = CLOCK_MONOTONIC;
	else
#endif
	if (clock_getres(CLOCK_REALTIME, &res) == 0)
		clocktouse = CLOCK_REALTIME;
	else
		return (-1);

	/* Convert clock resolution to a double. */
	*resd = res.tv_sec + res.tv_nsec * 0.000000001;

	return (0);
}

static int
getclocktime(struct timespec * ts)
{

	if (clock_gettime(clocktouse, ts))
		return (-1);

	return (0);
}

#else
static int
getclockres(double * resd)
{

	*resd = 1.0 / CLOCKS_PER_SEC;

	return (0);
}

static int
getclocktime(struct timespec * ts)
{
	struct timeval tv;

	if (gettimeofday(&tv, NULL))
		return (-1);
	ts->tv_sec = tv.tv_sec;
	ts->tv_nsec = tv.tv_usec * 1000;

	return (0);
}
#endif

static int
getclockdiff(struct timespec * st, double * diffd)
{
	struct timespec en;

	if (getclocktime(&en))
		return (1);
	*diffd = (en.tv_nsec - st->tv_nsec) * 0.000000001 +
	    (en.tv_sec - st->tv_sec);

	return (0);
}

/**
 * scryptenc_cpuperf(opps):
 * Estimate the number of salsa20/8 cores which can be executed per second,
 * and return the value via opps.
 */
int
scryptenc_cpuperf(double * opps)
{
	struct timespec st;
	double resd, diffd;
	uint64_t i = 0;

	/* Get the clock resolution. */
	if (getclockres(&resd))
		return (2);

#ifdef DEBUG
	fprintf(stderr, "Clock resolution is %f\n", resd);
#endif

	/* Loop until the clock ticks. */
	if (getclocktime(&st))
		return (2);
	do {
		/* Do an scrypt. */
		if (crypto_scrypt(NULL, 0, NULL, 0, 16, 1, 1, NULL, 0))
			return (3);

		/* Has the clock ticked? */
		if (getclockdiff(&st, &diffd))
			return (2);
		if (diffd > 0)
			break;
	} while (1);

	/* Count how many scrypts we can do before the next tick. */
	if (getclocktime(&st))
		return (2);
	do {
		/* Do an scrypt. */
		if (crypto_scrypt(NULL, 0, NULL, 0, 128, 1, 1, NULL, 0))
			return (3);

		/* We invoked the salsa20/8 core 512 times. */
		i += 512;

		/* Check if we have looped for long enough. */
		if (getclockdiff(&st, &diffd))
			return (2);
		if (diffd > resd)
			break;
	} while (1);

#ifdef DEBUG
	fprintf(stderr, "%ju salsa20/8 cores performed in %f seconds\n",
	    (uintmax_t)i, diffd);
#endif

	/* We can do approximately i salsa20/8 cores per diffd seconds. */
	*opps = i / diffd;
	return (0);
}
