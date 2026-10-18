#!/usr/bin/env python3
"""Package an existing Windows release build; does not build or publish it."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path, help="New .zip path (refuses overwrite)")
args = parser.parse_args()
exe = root / "target/x86_64-pc-windows-msvc/release/plico.exe"
if not exe.is_file() or exe.read_bytes()[:2] != b"MZ":
    raise SystemExit("Build the Windows release executable first")
if args.output.exists():
    raise SystemExit(f"Refusing to overwrite {args.output}")
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--offline", "--format-version", "1",
     "--filter-platform", "x86_64-pc-windows-msvc"], cwd=root,
))
helper = exe.with_name("gitui-askpass.exe")
if not helper.is_file() or helper.read_bytes()[:2] != b"MZ":
    raise SystemExit("Build the Windows AskPass helper first")
editor = exe.with_name("gitui-editor.exe")
if not editor.is_file() or editor.read_bytes()[:2] != b"MZ":
    raise SystemExit("Build the Windows editor helper first")
files = {"Plico.exe": exe, "gitui-askpass.exe": helper, "gitui-editor.exe": editor, "LICENSE.txt": root / "LICENSE",
         "README.txt": root / "docs/WINDOWS-PREVIEW.txt",
         "THIRD-PARTY.txt": root / "docs/THIRD-PARTY.txt"}
for directory in ["fonts", "icons", "themes", "branding"]:
    for path in (root / "assets" / directory).glob("*.txt"):
        files[f"licenses/assets/{directory}/{path.name}"] = path
packages = []
for package in metadata["packages"]:
    folder = Path(package["manifest_path"]).parent
    package_files = []
    for path in sorted(folder.iterdir()):
        if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
            destination = f"licenses/crates/{package['name']}-{package['version']}/{path.name}"
            files[destination] = path
            package_files.append(destination)
    packages.append({"name": package["name"], "version": package["version"],
                     "license": package["license"], "repository": package["repository"],
                     "included_license_files": package_files})
manifest = {"format": 1, "kind": "development-preview", "target": "x86_64-pc-windows-msvc",
            "executable_sha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
            "askpass_sha256": hashlib.sha256(helper.read_bytes()).hexdigest(),
            "editor_sha256": hashlib.sha256(editor.read_bytes()).hexdigest(),
            "dependencies": packages}
args.output.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(args.output, "x", compression=zipfile.ZIP_DEFLATED, strict_timestamps=False) as archive:
    for name, path in sorted(files.items()):
        archive.write(path, f"Plico-preview/{name}")
    archive.writestr("Plico-preview/manifest.json", json.dumps(manifest, indent=2))
with zipfile.ZipFile(args.output) as archive:
    if archive.testzip() is not None:
        raise SystemExit("Archive validation failed")
print(json.dumps({"archive": str(args.output), "bytes": args.output.stat().st_size,
                  "executable_sha256": manifest["executable_sha256"]}))
