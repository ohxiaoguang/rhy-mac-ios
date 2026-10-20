#!/usr/bin/env python3
"""Report measurable properties of the PCM actually sent to Core Audio."""
import argparse
import array
import json
import math
import sys
import wave
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('wav', type=Path)
p.add_argument('--out', type=Path)
args = p.parse_args()
with wave.open(str(args.wav), 'rb') as source:
    if source.getsampwidth() != 2:
        raise SystemExit('Expected PCM16')
    rate, channels = source.getframerate(), source.getnchannels()
    samples = array.array('h', source.readframes(source.getnframes()))
if sys.byteorder != 'little':
    samples.byteswap()
seconds = []
for offset in range(0, len(samples), rate * channels):
    block = samples[offset:offset + rate * channels]
    seconds.append({
        'second': offset // (rate * channels),
        'rms': round(math.sqrt(sum(x * x for x in block) / len(block)), 3),
        'peak': max(map(abs, block), default=0),
    })
report = {
    'file': str(args.wav), 'sample_rate': rate, 'channels': channels,
    'duration_seconds': len(samples) / (rate * channels),
    'peak': max(map(abs, samples), default=0),
    'clipping_fraction': sum(x in (-32768, 32767) for x in samples) / max(1, len(samples)),
    'nonzero_fraction': sum(x != 0 for x in samples) / max(1, len(samples)),
    'seconds': seconds,
}
data = json.dumps(report, indent=2)
if args.out:
    args.out.write_text(data + '\n')
print(json.dumps({k: v for k, v in report.items() if k != 'seconds'}, indent=2))
if report['peak'] == 0:
    raise SystemExit('FAIL: the output is entirely silent')
