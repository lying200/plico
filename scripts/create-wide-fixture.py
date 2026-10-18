"""Generate 32 branches and an octopus merge for graph scrolling validation.

Usage: python3 scripts/create-wide-fixture.py NEW_DIRECTORY
"""
import pathlib
import subprocess
import sys

root = pathlib.Path(sys.argv[1]).resolve()
if root.exists():
    raise SystemExit("Target already exists")
root.mkdir(parents=True)
subprocess.run(["git", "-C", str(root), "init", "-b", "main"], check=True)
process = subprocess.Popen(
    ["git", "-C", str(root), "fast-import", "--quiet"], stdin=subprocess.PIPE
)


def emit(value):
    process.stdin.write(value.encode())


emit("commit refs/heads/main\nmark :1\n")
emit("committer Wide Graph <wide@example.invalid> 1700000000 +0000\n")
emit("data 4\nRoot\n\n")
for step in range(32):
    for branch in range(32):
        message = f"Branch {branch:02d} step {step:02d}: review interleaved development"
        content = f"Branch {branch:02d}\nStep {step:02d}\n"
        emit(f"commit refs/heads/feature/{branch:02d}\n")
        emit(f"committer Developer {branch:02d} <dev{branch}@example.invalid> "
             f"{1700000001 + step * 32 + branch} +0000\n")
        emit(f"data {len(message)}\n{message}\n")
        if step == 0:
            emit("from :1\n")
        emit(f"M 100644 inline review.txt\ndata {len(content)}\n{content}\n\n")
message = "Merge 32 parallel development branches"
emit("commit refs/heads/main\n")
emit("committer Wide Graph <wide@example.invalid> 1700002000 +0000\n")
emit(f"data {len(message)}\n{message}\nfrom refs/heads/feature/00\n")
for branch in range(1, 32):
    emit(f"merge refs/heads/feature/{branch:02d}\n")
emit("\ndone\n")
process.stdin.close()
if process.wait():
    raise SystemExit("fast-import failed")
subprocess.run(["git", "-C", str(root), "reset", "--hard", "HEAD"],
               check=True, stdout=subprocess.DEVNULL)
print(root)
