import json,os,subprocess
from pathlib import Path
p=Path("preservation/m7-review")
a=json.loads((p/"invocation.json").read_text())
env=os.environ.copy();env.update(TMPDIR="/var/tmp/conglobate-rewrite-tmp",CARGO_BUILD_JOBS="2")
with (p/"review-stream.jsonl").open("w") as out,(p/"review-stderr.log").open("w") as err:
 r=subprocess.run(a["argv"],input=a["stdin"],text=True,stdout=out,stderr=err,env=env)
(p/"exit.json").write_text(json.dumps({"exit_code":r.returncode})+"\n")
print("Claude review exited",r.returncode)
raise SystemExit(r.returncode)
