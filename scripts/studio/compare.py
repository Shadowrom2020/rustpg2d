#!/usr/bin/env python3
"""Compares two folders of the renderer comparison set: per picture the share of pixels that differ
by more than a little, and a side-by-side sheet of the worst. usage: compare.py BASE NEW OUT.png"""
import sys, glob, os
from PIL import Image, ImageChops
base, new, out = sys.argv[1:4]
rows = []
for f in sorted(glob.glob(os.path.join(base, '*.png'))):
    n = os.path.join(new, os.path.basename(f))
    if not os.path.exists(n):
        print(f'{os.path.basename(f):18} MISSING'); continue
    a = Image.open(f).convert('RGB'); b = Image.open(n).convert('RGB')
    if a.size != b.size:
        print(f'{os.path.basename(f):18} size {a.size} vs {b.size}'); continue
    d = ImageChops.difference(a, b).convert('L')
    h = d.histogram()
    total = a.size[0] * a.size[1]
    over8 = sum(h[9:]) / total * 100
    over32 = sum(h[33:]) / total * 100
    print(f'{os.path.basename(f):18} >8: {over8:6.2f}%   >32: {over32:6.2f}%   max {max(i for i, c in enumerate(h) if c)}')
    rows.append((over8, a, b, d))
rows.sort(key=lambda r: -r[0])
rows = rows[:6]
if rows:
    w, h = rows[0][1].size
    tw = 426; th = h * tw // w
    sheet = Image.new('RGB', (tw * 3, th * len(rows)))
    for i, (_, a, b, d) in enumerate(rows):
        sheet.paste(a.resize((tw, th)), (0, i * th)); sheet.paste(b.resize((tw, th)), (tw, i * th))
        sheet.paste(d.point(lambda v: min(255, v * 4)).convert('RGB').resize((tw, th)), (tw * 2, i * th))
    sheet.save(out)
