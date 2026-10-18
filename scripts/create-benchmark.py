"""Generate commits (default 10,000); the tip adds a ~1 MiB text diff.

Usage: create-benchmark.py NEW_DIRECTORY [COMMIT_COUNT] [TIP_FILE_COUNT]
"""
import pathlib, subprocess, sys
root=pathlib.Path(sys.argv[1]).resolve()
count=int(sys.argv[2]) if len(sys.argv)>2 else 10000
files=int(sys.argv[3]) if len(sys.argv)>3 else 2
if not 2 <= files <= 1000: raise SystemExit('Tip file count must be 2..1000')
if count < 1: raise SystemExit('Commit count must be positive')
if root.exists(): raise SystemExit('Target already exists')
root.mkdir(parents=True)
subprocess.run(['git','-C',str(root),'init','-b','main'],check=True)
process=subprocess.Popen(['git','-C',str(root),'fast-import','--quiet'],stdin=subprocess.PIPE)
def emit(value): process.stdin.write(value.encode() if isinstance(value,str) else value)
for i in range(count):
    msg=f'Commit {i+1:05d}: review cache behavior'
    data=(f'Checkpoint {i+1}\n').encode()
    emit(f'commit refs/heads/main\ncommitter Benchmark <bench@example.invalid> {1700000000+i} +0000\ndata {len(msg)}\n{msg}\nM 100644 inline checkpoint.txt\ndata {len(data)}\n')
    emit(data); emit('\n')
    if i==count-1:
        data=''.join(f'pub const ENTRY_{j:05d}: &str = "bounded cache review text for entry {j:05d}";\n' for j in range(13000)).encode()
        emit(f'M 100644 inline review.rs\ndata {len(data)}\n');emit(data);emit('\n')
        for number in range(files - 2):
            data=''.join(f'pub const FILE_{number:03d}_LINE_{j:04d}: usize = {j};\n' for j in range(1000)).encode()
            emit(f'M 100644 inline file-{number:03d}.rs\ndata {len(data)}\n');emit(data);emit('\n')
    emit('\n')
emit('done\n');process.stdin.close()
if process.wait(): raise SystemExit('fast-import failed')
subprocess.run(['git','-C',str(root),'reset','--hard','HEAD'],check=True,stdout=subprocess.DEVNULL)
print(root)
