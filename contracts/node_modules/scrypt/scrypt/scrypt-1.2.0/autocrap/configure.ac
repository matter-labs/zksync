m4_define([SCRYPT_VERS],m4_include([scrypt-version]))
AC_INIT([scrypt],SCRYPT_VERS())
AC_CONFIG_AUX_DIR([config.aux])
AM_INIT_AUTOMAKE()
AC_PROG_CC
AC_PROG_RANLIB

# Don't rebuild the configure script.  I'm distributing a perfectly good one.
AM_MAINTAINER_MODE

# Check for clock_gettime.  On some systems, this is provided via librt.
AC_CHECK_LIB(rt, clock_gettime)
AC_CHECK_FUNCS([clock_gettime])

# Check for a linuxy sysinfo syscall; and while we're doing that, check if
# struct sysinfo is the old version (total RAM == totalmem) or the new
# version (total RAM == totalmem * mem_unit).
AC_CHECK_HEADERS([sys/sysinfo.h])
AC_CHECK_FUNCS([sysinfo])
AC_CHECK_TYPES([struct sysinfo], [], [], [[#include <sys/sysinfo.h>]])
AC_CHECK_MEMBERS([struct sysinfo.totalram, struct sysinfo.mem_unit], [], [],
    [[#include <sys/sysinfo.h>]])

# Check if we have <sys/param.h>, since some systems require it for sysctl
# to work.
AC_CHECK_HEADERS([sys/param.h])

# Check for <sys/sysctl.h>.  If it exists and it defines HW_USERMEM
# and/or HW_MEMSIZE, we'll try using those as memory limits.
AC_CHECK_HEADERS([sys/sysctl.h])

# Check for posix_memalign
AC_CHECK_FUNCS([posix_memalign])

# Check for mmap so we can work around its absence on Minix
AC_CHECK_FUNCS([mmap])

AC_SYS_LARGEFILE

AC_CONFIG_HEADERS([config.h])
AC_CONFIG_FILES([Makefile])
AC_OUTPUT
