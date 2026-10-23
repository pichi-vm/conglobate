"""Resume the preserved review with shell access and unchanged review constraints.

Usage: python3 preservation/m7-review/resume.py PROMPT_FILE NEW_OUTPUT_DIRECTORY
The original review must have exited first. Output directories are never reused.
"""

import json
import os
from pathlib import Path
import subprocess
import sys

if len(sys.argv) != 3:
    raise SystemExit(__doc__)

review = Path(__file__).resolve().parent
if not (review / "exit.json").is_file():
    raise SystemExit("Original review is still running; do not fork its session.")

original = json.loads((review / "invocation.json").read_text())
argv = original["argv"].copy()
argv[argv.index("--session-id")] = "--resume"
# dontAsk denies tools not explicitly allowed. Permit Bash itself so compound
# and environment-prefixed rootless checks work. This is not a shell sandbox;
# the preserved system instructions still prohibit edits and privileged work.
argv[argv.index("--allowedTools") + 1 :] = [
    "Read", "Glob", "Grep", "Agent", "Task", "Skill", "Bash"
]
prompt = Path(sys.argv[1]).read_text()
output = Path(sys.argv[2])
output.mkdir(parents=True, exist_ok=False)
(output / "invocation.json").write_text(json.dumps({
    "argv": argv,
    "stdin": prompt,
    "permission_correction": "Allow Bash, including compound and env-prefixed commands; retain review constraints.",
}, indent=2) + "\n")
env = os.environ.copy()
env.update(TMPDIR="/var/tmp/conglobate-rewrite-tmp", CARGO_BUILD_JOBS="2")
with (output / "review-stream.jsonl").open("w") as stdout, (output / "review-stderr.log").open("w") as stderr:
    result = subprocess.run(argv, input=prompt, text=True, stdout=stdout, stderr=stderr, env=env)
(output / "exit.json").write_text(json.dumps({"exit_code": result.returncode}) + "\n")
raise SystemExit(result.returncode)
