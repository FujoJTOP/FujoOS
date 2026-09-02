typedef long i64;
extern long sy(long,long,long,long,long,long);
static const char MSG[] = "tcc-compiled hello from fujo!\n";
void _start(void) {
  sy(1,1,(long)MSG,sizeof(MSG)-1,0,0);
  sy(60,7,0,0,0,0);
  for(;;){}
}
