#!/usr/bin/env python3
"""Archive the Mac bundle and record hashes of the tested local build."""
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
build = root / 'build'
subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent',
                str(build / 'Rhythm.app'), str(build / 'Rhythm-macOS-arm64.zip')], check=True)
binary = root / 'target/ios-xcode/Build/Products/Release-iphoneos/Rhythm.app/nxhost'
commands = subprocess.check_output(['otool', '-l', str(binary)], text=True)
(build / 'ios-load-commands.txt').write_text(commands)
segments, segment = [], {}
for line in commands.splitlines():
    parts = line.split()
    if len(parts) == 2 and parts[0] == 'segname':
        segment = {'segname': parts[1]}
        if parts[1].startswith('__NX'):
            segments.append(segment)
    elif len(parts) == 2 and parts[0] in ('vmaddr', 'vmsize', 'maxprot', 'initprot'):
        segment[parts[0]] = parts[1]
segments = [s for s in segments if 'initprot' in s]
assert len(segments) == 6, segments
assert all(int(s['initprot'], 16) & 6 != 6 for s in segments), 'Writable executable game segment'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

artifacts = {}
for name in ('Rhythm-macOS-arm64.zip', 'Rhythm.ipa'):
    p = build / name
    artifacts[name] = {'bytes': p.stat().st_size, 'sha256': digest(p)}
sources = []
for directory, pattern in [('crates', '*.rs'), ('tools', '*.py'), ('launcher', '*.swift'), ('ios', '*.swift')]:
    sources.extend((root / directory).rglob(pattern))
sources.extend(root / p for p in ['Cargo.toml', 'Cargo.lock', 'ios/project.yml', 'ios/Info.plist', 'ios/Rhythm.entitlements'])
manifest = {
    'version': '0.3.0',
    'status': 'launcher/input update; see docs/VALIDATION.md for tested versions',
    'modules': json.loads((root / 'work/game/patched/game.json').read_text()),
    'ios_segments': segments, 'artifacts': artifacts,
    'source_sha256': {str(p.relative_to(root)): digest(p) for p in sorted(set(sources))},
}
device_report = build / 'ipad-validation.json'
if device_report.exists():
    report = json.loads(device_report.read_text())
    if report.get('native_executable_sha256') == digest(binary):
        manifest['device_validation'] = report
        if report.get('acceptance', {}).get('status') == 'passed_by_user':
            manifest['status'] = 'macOS first stage tested; iOS accepted by user after manual testing'
(build / 'build-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(artifacts, indent=2))
