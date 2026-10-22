/* Trusted, guest-only test loader. Fixed fixture UID mapping; not production. */
#include <bpf/libbpf.h>
#include <bpf/bpf.h>
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <unistd.h>
struct config {unsigned int userns,dev,root_uid;};
static const char *names[]={"check_set","check_remove","skip_label_cap"};
static volatile sig_atomic_t stop;
static void done(int sig) { (void)sig; stop=1; }
static int clear(const char *dir) {
 char path[4096];snprintf(path,sizeof(path),"%s/config",dir);
 int fd=bpf_obj_get(path);unsigned int key=0;struct config zero={0};
 if(fd<0||bpf_map_update_elem(fd,&key,&zero,BPF_ANY)) {perror("disable policy");return 1;}
 close(fd);
 for(int i=2;i>=0;i--) {snprintf(path,sizeof(path),"%s/%s",dir,names[i]);if(unlink(path))return 1;}
 snprintf(path,sizeof(path),"%s/config",dir);return unlink(path)!=0;
}
int main(int argc,char **argv) {
 if(argc==3&&!strcmp(argv[1],"--clear"))return clear(argv[2]);
 if(argc!=6)return 2;
 dev_t dev=(dev_t)strtoull(argv[3],0,10);
 struct config c={strtoul(argv[2],0,10),(major(dev)<<20)|minor(dev),100000};
 struct bpf_object *o=bpf_object__open_file(argv[1],NULL);
 if(libbpf_get_error(o)||bpf_object__load(o))return 4;
 struct bpf_map *map=bpf_object__find_map_by_name(o,"config");
 if(!map)return 5;
 int fd=bpf_map__fd(map);unsigned int key=0;char path[4096];
 snprintf(path,sizeof(path),"%s/config",argv[5]);if(bpf_map__pin(map,path))return 5;
 struct bpf_link *links[3];int n=0;
 /* Config is initially disabled. Pin every guard before enabling the exemption.
  * Pins keep the full policy intact if this loader is killed unexpectedly. */
 for(int i=0;i<3;i++) {
  struct bpf_program *p=bpf_object__find_program_by_name(o,names[i]);if(!p)return 6;
  links[n]=bpf_program__attach_lsm(p);
  if(libbpf_get_error(links[n]))return 7;
  snprintf(path,sizeof(path),"%s/%s",argv[5],names[i]);if(bpf_link__pin(links[n],path))return 7;
  n++;
 }
 if(bpf_map_update_elem(fd,&key,&c,BPF_ANY))return 5;
 signal(SIGTERM,done);signal(SIGINT,done);signal(SIGALRM,done);alarm(240);
 FILE *ready=fopen(argv[4],"w");if(!ready)return 8;
 fprintf(ready,"%u %u\n",c.userns,c.dev);fclose(ready);
 printf("LABEL_POLICY_ATTACHED userns=%u dev=%u\n",c.userns,c.dev);fflush(stdout);
 while(!stop)pause();
 /* This only releases this process's references. Trusted coordinator clears
  * the map and pins after command exit, while holding namespace lifetime. */
 while(n) {bpf_link__disconnect(links[n-1]);bpf_link__destroy(links[--n]);}
 bpf_object__close(o);return 0;
}
