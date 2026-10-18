#!/usr/bin/env python3
"""Extract Rebased's Apache-2.0 PinyinMatcher tables without a JVM dependency.
Usage: python3 scripts/generate-pinyin-reference.py /path/to/rebased
"""
import json
import pathlib
import re
import struct
import subprocess
import sys
COMMIT = '4aec35256d765c217aa0d662b82ac4e860c396fd'
SOURCE = 'platform/util/text-matching/src/com/intellij/psi/codeStyle/PinyinMatcher.kt'
text = subprocess.check_output(['git', '-C', sys.argv[1], 'show', f'{COMMIT}:{SOURCE}'], text=True)
def strings(section):
    return ''.join(json.loads(s.replace(r'\$', '$')) for s in re.findall(r'"(?:[^"\\]|\\.)*"', section))
encoding = strings(text.split('val ENCODING: String =', 1)[1].split('@ApiStatus.Internal', 1)[0]).split(',')
data = strings(text.split('val DATA: String =', 1)[1].split('private val END_CODE_POINT', 1)[0])
assert len(data) == 0xA000 - 0x3400
assert all(ord(c) < 33 or ord(c)-33 < len(encoding) for c in data)
assert all(s and all('a' <= c <= 'z' for c in s) for s in encoding)
root = pathlib.Path(__file__).resolve().parents[1] / 'assets' / 'search'
root.mkdir(parents=True, exist_ok=True)
(root / 'pinyin-indices.bin').write_bytes(data.encode('utf-16-le'))
masks = [sum(1 << (ord(c)-ord('a')) for c in set(s)) for s in encoding]
(root / 'pinyin-masks.bin').write_bytes(b''.join(struct.pack('<I', mask) for mask in masks))
(root / 'SOURCE.txt').write_text(f'''Pinyin lookup tables derived from detachhead/rebased {COMMIT}
{SOURCE}
Copyright 2000-2025 JetBrains s.r.o. and contributors. Apache-2.0.
Indices are the original DATA UTF-16 units in little-endian order.
Masks encode each ENCODING entry's lowercase ASCII letters as bits 0..25.
Regenerate using scripts/generate-pinyin-reference.py and the pinned checkout.
{len(data)} code points starting at U+3400; {len(encoding)} encoding entries.
''')
print(f'{len(data)} characters; {len(encoding)} encodings; {len(data)*2+len(masks)*4} static bytes')
