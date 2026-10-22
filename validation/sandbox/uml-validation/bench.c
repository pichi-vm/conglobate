/* Bounded deterministic workload. Operates only in a supplied disposable cwd. */
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/xattr.h>
#include <time.h>
#include <unistd.h>
static double now(void) { struct timespec t; if(clock_gettime(CLOCK_MONOTONIC,&t))exit(2); return t.tv_sec+t.tv_nsec/1e9; }
static void check(int v) { if(v<0) {perror("benchmark");exit(2);} }
int main(void) {
 double t=now(); uint64_t h=1469598103934665603ULL;
 for(unsigned k=0;k<32;k++)for(unsigned j=0;j<4*1024*1024;j++)h=(h^(j&255))*1099511628211ULL;
 printf("BENCH cpu %.6f %llu\n",now()-t,(unsigned long long)h);fflush(stdout);
 t=now();char name[64],other[64];char data[4096];memset(data,42,sizeof(data));
 for(unsigned j=0;j<1000;j++) {
  snprintf(name,sizeof(name),"file-%u",j);snprintf(other,sizeof(other),"link-%u",j);
  int fd=open(name,O_CREAT|O_EXCL|O_WRONLY,0640);check(fd);
  if(write(fd,data,sizeof(data))!=sizeof(data))return 2;
  check(fchmod(fd,0600));check(fchown(fd,1234,2345));
  check(fsetxattr(fd,"user.validation","value",5,0));check(close(fd));
  check(link(name,other));check(unlink(other));check(unlink(name));
 }
 printf("BENCH metadata %.6f\n",now()-t);fflush(stdout);
 t=now();int fd=open("io",O_CREAT|O_EXCL|O_RDWR,0600);check(fd);
 for(unsigned j=0;j<16384;j++)if(write(fd,data,sizeof(data))!=sizeof(data))return 2;
 check(fsync(fd));check(lseek(fd,0,SEEK_SET));
 for(unsigned j=0;j<16384;j++)if(read(fd,data,sizeof(data))!=sizeof(data))return 2;
 check(close(fd));check(unlink("io"));
 printf("BENCH io %.6f\n",now()-t);
 struct rusage u;check(getrusage(RUSAGE_SELF,&u));printf("BENCH maxrss_kib %ld\n",u.ru_maxrss);
 puts("BENCH_PASS");return 0;
}
