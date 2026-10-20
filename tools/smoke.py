#!/usr/bin/env python3
"""Bounded native launch, optional thread sample, and automatic process cleanup."""
import argparse
import os
import plistlib
import shutil
from pathlib import Path
import subprocess
import time
os.chdir(Path(__file__).resolve().parent.parent)
p=argparse.ArgumentParser();p.add_argument('--seconds',type=int,default=25);p.add_argument('--sample',action='store_true');p.add_argument('--bundle',action='store_true');args=p.parse_args()
Path('build').mkdir(exist_ok=True)
env=os.environ.copy();env.update(NX_PHONE_PADS='0');env.setdefault('NX_SHADER_CACHE','work/shaders-macosx');env.setdefault('NX_DUMP_AT','3,10');env.setdefault('NX_TRACE_FRAME','1')
executable='./target/release/nxhost'
if args.bundle:
    app=Path('build/SmokeGame.app');folder=app/'Contents/MacOS';folder.mkdir(parents=True,exist_ok=True)
    shutil.copy2(executable,folder/'nxhost')
    with (app/'Contents/Info.plist').open('wb') as info:
        plistlib.dump({'CFBundleIdentifier':'dev.local.rhythm.smoke','CFBundleName':'SmokeGame','CFBundleExecutable':'nxhost','CFBundlePackageType':'APPL','NSHighResolutionCapable':True},info)
    subprocess.run(['codesign','--force','--sign','-',app],check=True)
    executable=str(folder/'nxhost')
with open('build/smoke-native.log','w') as log:
    stop_file=Path('work/stop-smoke')
    stop_file.unlink(missing_ok=True)
    child=subprocess.Popen([executable],stdout=log,stderr=log,env=env)
    try:
        if args.sample:
            time.sleep(min(10,args.seconds))
            if child.poll() is None:
                subprocess.run(['sample',str(child.pid),'2','-file','build/threads.txt'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=8)
        try:
            deadline=time.monotonic()+max(1,args.seconds-(10 if args.sample else 0))
            while child.poll() is None and time.monotonic()<deadline and not stop_file.exists():
                time.sleep(.25)
            if child.poll() is None:raise subprocess.TimeoutExpired(child.args,args.seconds)
            rc=child.returncode
            print(f'Game exited: {rc}');raise SystemExit(rc if rc>0 else 1)
        except subprocess.TimeoutExpired:
            print(f'Process survived the bounded launch. Check build/smoke-native.log and rendered images; survival is not gameplay verification.')
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=5)
