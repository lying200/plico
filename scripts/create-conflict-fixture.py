"""Create a disposable real index conflict; refuses to overwrite a directory.

Usage: python3 scripts/create-conflict-fixture.py NEW_DIRECTORY
After inspecting conflicts, run `git -C NEW_DIRECTORY merge --abort` externally
and refresh the viewer to verify that its conflict state clears.
"""
import pathlib
import subprocess
import sys

root = pathlib.Path(sys.argv[1]).resolve()
root.mkdir(parents=True, exist_ok=False)


def git(*args, check=True):
    return subprocess.run(
        ["git", "-C", str(root), *args], check=check, capture_output=True, text=True
    )


git("init", "-b", "main")
git("config", "user.name", "Conflict Fixture")
git("config", "user.email", "fixture@example.invalid")
git("config", "core.autocrlf", "false")
file = root / "review.txt"
file.write_text("original\n", encoding="utf-8")
git("add", "review.txt")
git("commit", "-m", "Base")
git("checkout", "-b", "feature")
file.write_text("feature implementation\n", encoding="utf-8")
git("commit", "-am", "Other developer implementation")
git("checkout", "main")
file.write_text("main implementation\n", encoding="utf-8")
git("commit", "-am", "Main implementation")
result = git("merge", "feature", check=False)
if result.returncode != 1 or not git("ls-files", "--unmerged").stdout:
    raise SystemExit(f"Expected real merge conflict: {result.stdout} {result.stderr}")
print(root)
print(git("status", "--short").stdout)
