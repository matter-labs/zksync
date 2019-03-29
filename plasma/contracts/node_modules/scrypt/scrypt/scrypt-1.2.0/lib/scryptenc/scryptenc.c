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

#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "crypto_aes.h"
#include "crypto_aesctr.h"
#include "crypto_entropy.h"
#include "insecure_memzero.h"
#include "sha256.h"
#include "sysendian.h"

#include "crypto_scrypt.h"
#include "memlimit.h"
#include "scryptenc_cpuperf.h"

#include "scryptenc.h"

#define ENCBLOCK 65536

static int pickparams(size_t, double, double,
    int *, uint32_t *, uint32_t *);
static int checkparams(size_t, double, double, int, uint32_t, uint32_t);

static int
pickparams(size_t maxmem, double maxmemfrac, double maxtime,
    int * logN, uint32_t * r, uint32_t * p)
{
	size_t memlimit;
	double opps;
	double opslimit;
	double maxN, maxrp;
	int rc;

	/* Figure out how much memory to use. */
	if (memtouse(maxmem, maxmemfrac, &memlimit))
		return (1);

	/* Figure out how fast the CPU is. */
	if ((rc = scryptenc_cpuperf(&opps)) != 0)
		return (rc);
	opslimit = opps * maxtime;

	/* Allow a minimum of 2^15 salsa20/8 cores. */
	if (opslimit < 32768)
		opslimit = 32768;

	/* Fix r = 8 for now. */
	*r = 8;

	/*
	 * The memory limit requires that 128Nr <= memlimit, while the CPU
	 * limit requires that 4Nrp <= opslimit.  If opslimit < memlimit/32,
	 * opslimit imposes the stronger limit on N.
	 */
#ifdef DEBUG
	fprintf(stderr, "Requiring 128Nr <= %zu, 4Nrp <= %f\n",
	    memlimit, opslimit);
#endif
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
		maxN = memlimit / (*r * 128);
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

#ifdef DEBUG
	fprintf(stderr, "N = %zu r = %d p = %d\n",
	    (size_t)(1) << *logN, (int)(*r), (int)(*p));
#endif

	/* Success! */
	return (0);
}

static int
checkparams(size_t maxmem, double maxmemfrac, double maxtime,
    int logN, uint32_t r, uint32_t p)
{
	size_t memlimit;
	double opps;
	double opslimit;
	uint64_t N;
	int rc;

	/* Figure out the maximum amount of memory we can use. */
	if (memtouse(maxmem, maxmemfrac, &memlimit))
		return (1);

	/* Figure out how fast the CPU is. */
	if ((rc = scryptenc_cpuperf(&opps)) != 0)
		return (rc);
	opslimit = opps * maxtime;

	/* Sanity-check values. */
	if ((logN < 1) || (logN > 63))
		return (7);
	if ((uint64_t)(r) * (uint64_t)(p) >= 0x40000000)
		return (7);

	/* Check limits. */
	N = (uint64_t)(1) << logN;
	if ((memlimit / N) / r < 128)
		return (9);
	if ((opslimit / N) / (r * p) < 4)
		return (10);

	/* Success! */
	return (0);
}

static int
scryptenc_setup(uint8_t header[96], uint8_t dk[64],
    const uint8_t * passwd, size_t passwdlen,
    size_t maxmem, double maxmemfrac, double maxtime)
{
	uint8_t salt[32];
	uint8_t hbuf[32];
	int logN;
	uint64_t N;
	uint32_t r;
	uint32_t p;
	SHA256_CTX ctx;
	uint8_t * key_hmac = &dk[32];
	HMAC_SHA256_CTX hctx;
	int rc;

	/* Pick values for N, r, p. */
	if ((rc = pickparams(maxmem, maxmemfrac, maxtime,
	    &logN, &r, &p)) != 0)
		return (rc);
	N = (uint64_t)(1) << logN;

	/* Get some salt. */
	if (crypto_entropy_read(salt, 32))
		return (4);

	/* Generate the derived keys. */
	if (crypto_scrypt(passwd, passwdlen, salt, 32, N, r, p, dk, 64))
		return (3);

	/* Construct the file header. */
	memcpy(header, "scrypt", 6);
	header[6] = 0;
	header[7] = logN;
	be32enc(&header[8], r);
	be32enc(&header[12], p);
	memcpy(&header[16], salt, 32);

	/* Add header checksum. */
	SHA256_Init(&ctx);
	SHA256_Update(&ctx, header, 48);
	SHA256_Final(hbuf, &ctx);
	memcpy(&header[48], hbuf, 16);

	/* Add header signature (used for verifying password). */
	HMAC_SHA256_Init(&hctx, key_hmac, 32);
	HMAC_SHA256_Update(&hctx, header, 64);
	HMAC_SHA256_Final(hbuf, &hctx);
	memcpy(&header[64], hbuf, 32);

	/* Success! */
	return (0);
}

static int
scryptdec_setup(const uint8_t header[96], uint8_t dk[64],
    const uint8_t * passwd, size_t passwdlen,
    size_t maxmem, double maxmemfrac, double maxtime)
{
	uint8_t salt[32];
	uint8_t hbuf[32];
	int logN;
	uint32_t r;
	uint32_t p;
	uint64_t N;
	SHA256_CTX ctx;
	uint8_t * key_hmac = &dk[32];
	HMAC_SHA256_CTX hctx;
	int rc;

	/* Parse N, r, p, salt. */
	logN = header[7];
	r = be32dec(&header[8]);
	p = be32dec(&header[12]);
	memcpy(salt, &header[16], 32);

	/* Verify header checksum. */
	SHA256_Init(&ctx);
	SHA256_Update(&ctx, header, 48);
	SHA256_Final(hbuf, &ctx);
	if (memcmp(&header[48], hbuf, 16))
		return (7);

	/*
	 * Check whether the provided parameters are valid and whether the
	 * key derivation function can be computed within the allowed memory
	 * and CPU time.
	 */
	if ((rc = checkparams(maxmem, maxmemfrac, maxtime, logN, r, p)) != 0)
		return (rc);

	/* Compute the derived keys. */
	N = (uint64_t)(1) << logN;
	if (crypto_scrypt(passwd, passwdlen, salt, 32, N, r, p, dk, 64))
		return (3);

	/* Check header signature (i.e., verify password). */
	HMAC_SHA256_Init(&hctx, key_hmac, 32);
	HMAC_SHA256_Update(&hctx, header, 64);
	HMAC_SHA256_Final(hbuf, &hctx);
	if (memcmp(hbuf, &header[64], 32))
		return (11);

	/* Success! */
	return (0);
}

/**
 * scryptenc_buf(inbuf, inbuflen, outbuf, passwd, passwdlen,
 *     maxmem, maxmemfrac, maxtime):
 * Encrypt inbuflen bytes from inbuf, writing the resulting inbuflen + 128
 * bytes to outbuf.
 */
int
scryptenc_buf(const uint8_t * inbuf, size_t inbuflen, uint8_t * outbuf,
    const uint8_t * passwd, size_t passwdlen,
    size_t maxmem, double maxmemfrac, double maxtime)
{
	uint8_t dk[64];
	uint8_t hbuf[32];
	uint8_t header[96];
	uint8_t * key_enc = dk;
	uint8_t * key_hmac = &dk[32];
	int rc;
	HMAC_SHA256_CTX hctx;
	struct crypto_aes_key * key_enc_exp;
	struct crypto_aesctr * AES;

	/* Generate the header and derived key. */
	if ((rc = scryptenc_setup(header, dk, passwd, passwdlen,
	    maxmem, maxmemfrac, maxtime)) != 0)
		return (rc);

	/* Copy header into output buffer. */
	memcpy(outbuf, header, 96);

	/* Encrypt data. */
	if ((key_enc_exp = crypto_aes_key_expand(key_enc, 32)) == NULL)
		return (5);
	if ((AES = crypto_aesctr_init(key_enc_exp, 0)) == NULL)
		return (6);
	crypto_aesctr_stream(AES, inbuf, &outbuf[96], inbuflen);
	crypto_aesctr_free(AES);
	crypto_aes_key_free(key_enc_exp);

	/* Add signature. */
	HMAC_SHA256_Init(&hctx, key_hmac, 32);
	HMAC_SHA256_Update(&hctx, outbuf, 96 + inbuflen);
	HMAC_SHA256_Final(hbuf, &hctx);
	memcpy(&outbuf[96 + inbuflen], hbuf, 32);

	/* Zero sensitive data. */
	insecure_memzero(dk, 64);

	/* Success! */
	return (0);
}

/**
 * scryptdec_buf(inbuf, inbuflen, outbuf, outlen, passwd, passwdlen,
 *     maxmem, maxmemfrac, maxtime):
 * Decrypt inbuflen bytes from inbuf, writing the result into outbuf and the
 * decrypted data length to outlen.  The allocated length of outbuf must
 * be at least inbuflen.
 */
int
scryptdec_buf(const uint8_t * inbuf, size_t inbuflen, uint8_t * outbuf,
    size_t * outlen, const uint8_t * passwd, size_t passwdlen,
    size_t maxmem, double maxmemfrac, double maxtime)
{
	uint8_t hbuf[32];
	uint8_t dk[64];
	uint8_t * key_enc = dk;
	uint8_t * key_hmac = &dk[32];
	int rc;
	HMAC_SHA256_CTX hctx;
	struct crypto_aes_key * key_enc_exp;
	struct crypto_aesctr * AES;

	/*
	 * All versions of the scrypt format will start with "scrypt" and
	 * have at least 7 bytes of header.
	 */
	if ((inbuflen < 7) || (memcmp(inbuf, "scrypt", 6) != 0))
		return (7);

	/* Check the format. */
	if (inbuf[6] != 0)
		return (8);

	/* We must have at least 128 bytes. */
	if (inbuflen < 128)
		return (7);

	/* Parse the header and generate derived keys. */
	if ((rc = scryptdec_setup(inbuf, dk, passwd, passwdlen,
	    maxmem, maxmemfrac, maxtime)) != 0)
		return (rc);

	/* Decrypt data. */
	if ((key_enc_exp = crypto_aes_key_expand(key_enc, 32)) == NULL)
		return (5);
	if ((AES = crypto_aesctr_init(key_enc_exp, 0)) == NULL)
		return (6);
	crypto_aesctr_stream(AES, &inbuf[96], outbuf, inbuflen - 128);
	crypto_aesctr_free(AES);
	crypto_aes_key_free(key_enc_exp);
	*outlen = inbuflen - 128;

	/* Verify signature. */
	HMAC_SHA256_Init(&hctx, key_hmac, 32);
	HMAC_SHA256_Update(&hctx, inbuf, inbuflen - 32);
	HMAC_SHA256_Final(hbuf, &hctx);
	if (memcmp(hbuf, &inbuf[inbuflen - 32], 32))
		return (7);

	/* Zero sensitive data. */
	insecure_memzero(dk, 64);

	/* Success! */
	return (0);
}

/**
 * scryptenc_file(infile, outfile, passwd, passwdlen,
 *     maxmem, maxmemfrac, maxtime):
 * Read a stream from infile and encrypt it, writing the resulting stream to
 * outfile.
 */
int
scryptenc_file(FILE * infile, FILE * outfile,
    const uint8_t * passwd, size_t passwdlen,
    size_t maxmem, double maxmemfrac, double maxtime)
{
	uint8_t buf[ENCBLOCK];
	uint8_t dk[64];
	uint8_t hbuf[32];
	uint8_t header[96];
	uint8_t * key_enc = dk;
	uint8_t * key_hmac = &dk[32];
	size_t readlen;
	HMAC_SHA256_CTX hctx;
	struct crypto_aes_key * key_enc_exp;
	struct crypto_aesctr * AES;
	int rc;

	/* Generate the header and derived key. */
	if ((rc = scryptenc_setup(header, dk, passwd, passwdlen,
	    maxmem, maxmemfrac, maxtime)) != 0)
		return (rc);

	/* Hash and write the header. */
	HMAC_SHA256_Init(&hctx, key_hmac, 32);
	HMAC_SHA256_Update(&hctx, header, 96);
	if (fwrite(header, 96, 1, outfile) != 1)
		return (12);

	/*
	 * Read blocks of data, encrypt them, and write them out; hash the
	 * data as it is produced.
	 */
	if ((key_enc_exp = crypto_aes_key_expand(key_enc, 32)) == NULL)
		return (5);
	if ((AES = crypto_aesctr_init(key_enc_exp, 0)) == NULL)
		return (6);
	do {
		if ((readlen = fread(buf, 1, ENCBLOCK, infile)) == 0)
			break;
		crypto_aesctr_stream(AES, buf, buf, readlen);
		HMAC_SHA256_Update(&hctx, buf, readlen);
		if (fwrite(buf, 1, readlen, outfile) < readlen) {
			crypto_aesctr_free(AES);
			return (12);
		}
	} while (1);
	crypto_aesctr_free(AES);
	crypto_aes_key_free(key_enc_exp);

	/* Did we exit the loop due to a read error? */
	if (ferror(infile))
		return (13);

	/* Compute the final HMAC and output it. */
	HMAC_SHA256_Final(hbuf, &hctx);
	if (fwrite(hbuf, 32, 1, outfile) != 1)
		return (12);

	/* Zero sensitive data. */
	insecure_memzero(dk, 64);

	/* Success! */
	return (0);
}

/**
 * scryptdec_file(infile, outfile, passwd, passwdlen,
 *     maxmem, maxmemfrac, maxtime):
 * Read a stream from infile and decrypt it, writing the resulting stream to
 * outfile.
 */
int
scryptdec_file(FILE * infile, FILE * outfile,
    const uint8_t * passwd, size_t passwdlen,
    size_t maxmem, double maxmemfrac, double maxtime)
{
	uint8_t buf[ENCBLOCK + 32];
	uint8_t header[96];
	uint8_t hbuf[32];
	uint8_t dk[64];
	uint8_t * key_enc = dk;
	uint8_t * key_hmac = &dk[32];
	size_t buflen = 0;
	size_t readlen;
	HMAC_SHA256_CTX hctx;
	struct crypto_aes_key * key_enc_exp;
	struct crypto_aesctr * AES;
	int rc;

	/*
	 * Read the first 7 bytes of the file; all future versions of scrypt
	 * are guaranteed to have at least 7 bytes of header.
	 */
	if (fread(header, 7, 1, infile) < 1) {
		if (ferror(infile))
			return (13);
		else
			return (7);
	}

	/* Do we have the right magic? */
	if (memcmp(header, "scrypt", 6))
		return (7);
	if (header[6] != 0)
		return (8);

	/*
	 * Read another 89 bytes of the file; version 0 of the scrypt file
	 * format has a 96-byte header.
	 */
	if (fread(&header[7], 89, 1, infile) < 1) {
		if (ferror(infile))
			return (13);
		else
			return (7);
	}

	/* Parse the header and generate derived keys. */
	if ((rc = scryptdec_setup(header, dk, passwd, passwdlen,
	    maxmem, maxmemfrac, maxtime)) != 0)
		return (rc);

	/* Start hashing with the header. */
	HMAC_SHA256_Init(&hctx, key_hmac, 32);
	HMAC_SHA256_Update(&hctx, header, 96);

	/*
	 * We don't know how long the encrypted data block is (we can't know,
	 * since data can be streamed into 'scrypt enc') so we need to read
	 * data and decrypt all of it except the final 32 bytes, then check
	 * if that final 32 bytes is the correct signature.
	 */
	if ((key_enc_exp = crypto_aes_key_expand(key_enc, 32)) == NULL)
		return (5);
	if ((AES = crypto_aesctr_init(key_enc_exp, 0)) == NULL)
		return (6);
	do {
		/* Read data until we have more than 32 bytes of it. */
		if ((readlen = fread(&buf[buflen], 1,
		    ENCBLOCK + 32 - buflen, infile)) == 0)
			break;
		buflen += readlen;
		if (buflen <= 32)
			continue;

		/*
		 * Decrypt, hash, and output everything except the last 32
		 * bytes out of what we have in our buffer.
		 */
		HMAC_SHA256_Update(&hctx, buf, buflen - 32);
		crypto_aesctr_stream(AES, buf, buf, buflen - 32);
		if (fwrite(buf, 1, buflen - 32, outfile) < buflen - 32) {
			crypto_aesctr_free(AES);
			return (12);
		}

		/* Move the last 32 bytes to the start of the buffer. */
		memmove(buf, &buf[buflen - 32], 32);
		buflen = 32;
	} while (1);
	crypto_aesctr_free(AES);
	crypto_aes_key_free(key_enc_exp);

	/* Did we exit the loop due to a read error? */
	if (ferror(infile))
		return (13);

	/* Did we read enough data that we *might* have a valid signature? */
	if (buflen < 32)
		return (7);

	/* Verify signature. */
	HMAC_SHA256_Final(hbuf, &hctx);
	if (memcmp(hbuf, buf, 32))
		return (7);

	/* Zero sensitive data. */
	insecure_memzero(dk, 64);

	return (0);
}
