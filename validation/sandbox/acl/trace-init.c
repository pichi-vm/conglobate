/* Disposable diagnostic interposer. Never a conglobate implementation. */
#define _GNU_SOURCE
#define FUSE_USE_VERSION 35
#include <fuse.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void *(*original_init)(struct fuse_conn_info *, struct fuse_config *);
static void *trace_init(struct fuse_conn_info *c, struct fuse_config *cfg) {
 fprintf(stderr,"INIT_BEFORE capable=%x want=%x capable_ext=%llx want_ext=%llx\n",c->capable,c->want,(unsigned long long)c->capable_ext,(unsigned long long)c->want_ext);
 void *p=original_init(c,cfg);
 fprintf(stderr,"INIT_AFTER capable=%x want=%x capable_ext=%llx want_ext=%llx\n",c->capable,c->want,(unsigned long long)c->capable_ext,(unsigned long long)c->want_ext);
 return p;
}
int fuse_main_real_versioned(int argc,char **argv,const struct fuse_operations *op,size_t n,struct libfuse_version *v,void *p) {
 fprintf(stderr,"APPLICATION_FUSE_VERSION %d.%d.%d op_size=%zu compiled_size=%zu\n",v->major,v->minor,v->hotfix,n,sizeof(*op));
 if(n>sizeof(*op))abort();
 struct fuse_operations copy={0};memcpy(&copy,op,n);original_init=copy.init;copy.init=trace_init;
 int (*real)(int,char **,const struct fuse_operations*,size_t,struct libfuse_version*,void*)=dlsym(RTLD_NEXT,"fuse_main_real_versioned");
 return real(argc,argv,&copy,n,v,p);
}
