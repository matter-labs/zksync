#ifndef _CPUSUPPORT_H_
#define _CPUSUPPORT_H_

/*
 * To enable support for non-portable CPU features at compile time, one or
 * more CPUSUPPORT_ARCH_FEATURE macros should be defined.  This can be done
 * directly on the compiler command line; or a file can be created with the
 * necessary #define lines and then -D CPUSUPPORT_CONFIG_FILE=cpuconfig.h
 * (or similar) can be provided to include that file here.
 */
#ifdef CPUSUPPORT_CONFIG_FILE
#include CPUSUPPORT_CONFIG_FILE
#endif

/*
 * The CPUSUPPORT_FEATURE macro declares the necessary variables and
 * functions for detecting CPU feature support at run time.  The function
 * defined in the macro acts to cache the result of the ..._detect function
 * using the ..._present and ..._init variables.
 */
#define CPUSUPPORT_FEATURE(arch, feature)				\
	extern int cpusupport_ ## arch ## _ ## feature ## _present;	\
	extern int cpusupport_ ## arch ## _ ## feature ## _init;	\
	int cpusupport_ ## arch ## _ ## feature ## _detect(void);	\
									\
	static inline int						\
	cpusupport_ ## arch ## _ ## feature(void)			\
	{								\
									\
		if (cpusupport_ ## arch ## _ ## feature ## _present)	\
			return (1);					\
		else if (cpusupport_ ## arch ## _ ## feature ## _init)	\
			return (0);					\
		cpusupport_ ## arch ## _ ## feature ## _present = 	\
		    cpusupport_ ## arch ##_ ## feature ## _detect();	\
		cpusupport_ ## arch ## _ ## feature ## _init = 1;	\
		return (cpusupport_ ## arch ## _ ## feature ## _present); \
	}								\
	struct cpusupport_ ## arch ## _ ## feature ## _dummy

/*
 * CPUSUPPORT_FEATURE_DECL(arch, feature):
 * Macro which defines variables and provides a function declaration for
 * detecting the presence of "feature" on the "arch" architecture.  The
 * function body following this macro expansion must return nonzero if the
 * feature is present, or zero if the feature is not present or the detection
 * fails for any reason.
 */
#define CPUSUPPORT_FEATURE_DECL(arch, feature)				\
	int cpusupport_ ## arch ## _ ## feature ## _present = 0;	\
	int cpusupport_ ## arch ## _ ## feature ## _init = 0;		\
	int								\
	cpusupport_ ## arch ## _ ## feature ## _detect(void)

/*
 * Any features listed here must have associated C files compiled and linked
 * in, since the macro references symbols which must be defined.  Projects
 * which do not need to detect certain CPU features may wish to remove lines
 * from this list so that the associated C files can be omitted.
 */
CPUSUPPORT_FEATURE(x86, aesni);
CPUSUPPORT_FEATURE(x86, sse2);

#endif /* !_CPUSUPPORT_H_ */
