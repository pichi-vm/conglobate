package main
import("os";"encoding/json";"encoding/binary";"crypto/sha256";"fmt"; mode "github.com/tonistiigi/dchapes-mode")
type Case struct { Mode string `json:"mode"`; Directory bool `json:"directory"`; Digest string `json:"digest"` }
func main(){
 patterns:=[]string{"u-x,g+X","a-x,a+X","u+x,g+X","u+t=rw","g=uw","g=wu","u=go","u=g,o=u-w","a=rw-t"}
 for _, who:=range []string{"","u","g","o","a","ug","go"}{ for _, op:=range []string{"+","-","="}{for _, p:=range []string{"","r","wx","X","s","t","rt","tr","u","g","o","ug","go","rwX","uX","Xu","uw","wu","ut","tu"}{patterns=append(patterns,who+op+p)}}}
 out:=[]Case{}
 for _, spec:=range patterns { set,err:=mode.ParseWithUmask(spec,0);if err!=nil{panic(err)}
 for _, dir:=range []bool{false,true}{h:=sha256.New();for n:=0;n<4096;n++{m:=os.FileMode(n&0777);if n&04000!=0{m|=os.ModeSetuid};if n&02000!=0{m|=os.ModeSetgid};if n&01000!=0{m|=os.ModeSticky};if dir{m|=os.ModeDir};v:=set.Apply(m);b:=uint32(v.Perm());if v&os.ModeSetuid!=0{b|=04000};if v&os.ModeSetgid!=0{b|=02000};if v&os.ModeSticky!=0{b|=01000};var data [4]byte;binary.BigEndian.PutUint32(data[:],b);h.Write(data[:])};out=append(out,Case{spec,dir,fmt.Sprintf("%x",h.Sum(nil))})}}
 if err:=json.NewEncoder(os.Stdout).Encode(out);err!=nil{panic(err)}
}
