#!/usr/bin/env python3
"""Render the user supplied SVG at native Windows icon sizes (requires rsvg-convert)."""
from pathlib import Path
import struct
import subprocess
root = Path(__file__).resolve().parent.parent
folder = root / 'assets/branding'
sizes = (16, 20, 24, 32, 40, 48, 64, 128, 256)
images = [subprocess.check_output(['rsvg-convert','-w',str(s),'-h',str(s),str(folder/'plico.svg')]) for s in sizes]
offset = 6 + 16 * len(images)
entries = []
for size, data in zip(sizes, images):
    entries.append(struct.pack('<BBBBHHII',size % 256,size % 256,0,0,1,32,len(data),offset))
    offset += len(data)
(folder/'plico.ico').write_bytes(struct.pack('<HHH',0,1,len(images)) + b''.join(entries) + b''.join(images))
(folder/'plico.png').write_bytes(images[-1])
