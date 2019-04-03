#ifndef _MY_GETTIMEOFDAY_H_
#define _MY_GETTIMEOFDAY_H_

#ifdef _MSC_VER

#include <winsock2.h>
int gettimeofday(struct timeval * tp, struct timezone * tzp);

#if _MSC_VER < 1900
struct timespec {
    time_t tv_sec;
    time_t tv_nsec;
};
#endif

#endif /* _MSC_VER */

#endif /* _MY_GETTIMEOFDAY_H_ */
