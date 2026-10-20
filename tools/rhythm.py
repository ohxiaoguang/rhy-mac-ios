#!/usr/bin/env python3
"""Local, reproducible preparation and packaging. Never reads keys into logs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parent.parent
os.chdir(ROOT)
os.environ['PATH'] = '/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:' + os.environ['PATH']
BUILD = ROOT / 'build'
WORK = ROOT / 'work/game'
PATCHED = WORK / 'patched'
TOOL = ROOT / 'target/release'
TITLE = '0100d9f01d474000'

def run(*args, env=None):
    print('+', ' '.join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True, env=env)

def configure(folder):
    folder = Path(folder).expanduser().resolve()
    games = sorted(folder.glob('*.xci'))
    if len(games) != 1: raise RuntimeError('游戏目录必须包含且仅包含一个 XCI')
    for name, source in [('base.xci', games[0]), ('prod.keys', folder/'prod.keys'), ('Firmware', folder/'Firmware')]:
        if not source.exists(): raise RuntimeError(f'缺少 {source}')
        dest = ROOT/name
        if dest.is_symlink(): dest.unlink()
        elif dest.exists(): raise RuntimeError(f'保留现有文件；请自行移走 {dest} 后重试')
        dest.symlink_to(source)
    print('已建立本地资源链接，原始文件保持在游戏目录中。')

def build_tools():
    run('cargo', 'build', '--locked', '--offline', '--release', '-p', 'nxfs', '-p', 'nxpatch', '-p', 'nxhost')

def modules():
    return sorted((WORK/'exefs').iterdir(), key=lambda p: (0 if p.name=='main' else 2 if p.name=='sdk' else 1, p.name))

def prepare():
    build_tools()
    if not (ROOT/'base.xci').is_file() or not (ROOT/'prod.keys').is_file():
        raise RuntimeError('先运行 ./build.sh configure /path/to/your/game-folder')
    info = subprocess.check_output([TOOL/'nxfs','info','base.xci'], text=True)
    if f'Program program={TITLE}' not in info: raise RuntimeError('XCI 标题不符，停止准备')
    WORK.mkdir(parents=True, exist_ok=True)
    (WORK/'container.txt').write_text(info)
    fingerprint = hashlib.sha256((info+str((ROOT/'base.xci').stat().st_size)).encode()).hexdigest()
    stamp = WORK/'source.stamp'
    if not stamp.exists() or stamp.read_text() != fingerprint:
        if (WORK/'exefs').exists(): shutil.rmtree(WORK/'exefs')
        if PATCHED.exists(): shutil.rmtree(PATCHED)
        run(TOOL/'nxfs','extract','--base','base.xci','--out',WORK)
        stamp.write_text(fingerprint)
    paths = [p for p in modules() if p.read_bytes()[:4] == b'NSO0' and p.name != 'rtld']
    if not paths or not any(p.name=='sdk' for p in paths): raise RuntimeError('未提取到 main/sdk')
    patch_stamp = subprocess.check_output([TOOL/'nxpatch','stamp'],text=True).strip()
    if not (PATCHED/'nxpatch.stamp').exists() or (PATCHED/'nxpatch.stamp').read_text().strip()!=patch_stamp:
        if PATCHED.exists(): shutil.rmtree(PATCHED)
        run(TOOL/'nxpatch','rewrite','--out',PATCHED,*paths)
    report = {'title_id':TITLE, 'version':'1.0.0', 'container_fingerprint':fingerprint, 'modules':{}}
    for p in paths:
        header = p.read_bytes()[:0x100]
        expected={'main':'03ff4a3bec96bde3a2e37ce3aa4caa0cae8935a8','sdk':'cff2680fe74cb7d8d13b525730d1a7fd9a3f36f0'}
        if p.name not in expected or header[0x40:0x54].hex()!=expected[p.name]: raise RuntimeError(f'不支持的游戏模块构建号：{p.name}')
        report['modules'][p.name] = {'build_id':header[0x40:0x54].hex(), 'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
        with (WORK/(p.name+'-symbols.txt')).open('w') as out:
            subprocess.run([TOOL/'nxfs','nso',p],check=True,stdout=out)
    (PATCHED/'game.json').write_text(json.dumps(report,indent=2)+'\n')
    print('程序提取、重定位解析与离线改写完成；不代表已验证游戏可玩。')

def shaders(sdk):
    prepare()
    env=os.environ.copy(); env['NX_SHADER_SDK']=sdk
    cache=ROOT/f'work/shaders-{sdk}';cache.mkdir(parents=True,exist_ok=True)
    env['NX_SHADER_CACHE']=str(cache)
    lines=[]
    for path in [ROOT/'data/pipelines.tsv',ROOT/'work/pipelines.tsv']:
        if path.exists(): lines.extend(path.read_text().splitlines())
    (cache/'pipelines.tsv').write_text('\n'.join(dict.fromkeys(lines))+'\n')
    env['NX_PIPELINE_LIST']=str(cache/'pipelines.tsv')
    run(TOOL/'nxhost','--precompile',env=env)

def macos():
    prepare()
    app=BUILD/'Rhythm.app'
    if app.exists(): shutil.rmtree(app)
    bin=app/'Contents/MacOS'; res=app/'Contents/Resources'; bin.mkdir(parents=True); res.mkdir()
    for name in ['nxhost','nxfs','nxpatch']: shutil.copy2(TOOL/name,bin/name)
    run('xcrun','swiftc','-O','-parse-as-library','-target','arm64-apple-macos14','-module-cache-path',ROOT/'target/ModuleCache','launcher/Launcher.swift','-o',bin/'Launcher')
    info={'CFBundleName':'Rhythm','CFBundleDisplayName':'Rhythm','CFBundleIdentifier':'dev.local.rhythm',
          'CFBundleExecutable':'Launcher','CFBundlePackageType':'APPL','CFBundleShortVersionString':'0.2.0',
          'CFBundleVersion':'2','LSMinimumSystemVersion':'14.0','NSHighResolutionCapable':True,
          'LSApplicationCategoryType':'public.app-category.music-games'}
    with (app/'Contents/Info.plist').open('wb') as out: plistlib.dump(info,out)
    shutil.copy2(PATCHED/'game.json',res/'game.json')
    cache=ROOT/'work/shaders-macosx'
    (res/'shaders-macosx').mkdir()
    for p in cache.iterdir():
        if p.suffix in ['.metallib','.tsv']: shutil.copy2(p,res/'shaders-macosx'/p.name)
    shutil.copy2(ROOT/'LICENSE',res/'LICENSE')
    shutil.copy2(ROOT/'third_party/nxnative/LICENSE',res/'nxnative-LICENSE')
    run('codesign','--force','--deep','--sign','-',app)
    run('codesign','--verify','--deep',app)
    print(app)

def ios(sign=False, device=None):
    if device and not sign:
        raise RuntimeError('--device 需要与 --signed 一起使用')
    prepare()
    env=os.environ.copy(); env['NX_MODULES_DIR']=str(PATCHED); env['IPHONEOS_DEPLOYMENT_TARGET']='26.0'
    run('cargo','+1.98.1','build','--locked','--offline','--release','-p','nxhost','--target','aarch64-apple-ios','--target-dir','target/ios',env=env)
    resources=ROOT/'target/ios-res'
    if resources.exists(): shutil.rmtree(resources)
    (resources/'modules').mkdir(parents=True);(resources/'shaders').mkdir()
    for p in PATCHED.glob('*.layout'): shutil.copy2(p,resources/'modules'/p.name)
    for src in [WORK/'exefs/main.npdm',PATCHED/'game.json']: shutil.copy2(src,resources/'modules'/src.name)
    cache=ROOT/'work/shaders-iphoneos'
    if not (cache/'index.tsv').is_file() or not (cache/'guest.tsv').is_file() or not list(cache.glob('*.metallib')):
        raise RuntimeError('缺少 iOS Metal 库；先执行 ./build.sh shaders iphoneos')
    if cache.exists():
        for p in cache.iterdir():
            if p.suffix in ['.metallib','.tsv']: shutil.copy2(p,resources/'shaders'/p.name)
    shutil.copy2(ROOT/'LICENSE',resources/'LICENSE')
    shutil.copy2(ROOT/'third_party/nxnative/LICENSE',resources/'nxnative-LICENSE')
    run('xcodegen','generate','--spec','ios/project.yml')
    args=['xcodebuild','-quiet','-project','ios/Rhythm.xcodeproj','-scheme','Rhythm','-configuration','Release',
          '-destination',f'platform=iOS,id={device}' if device else 'generic/platform=iOS','-derivedDataPath','target/ios-xcode','build']
    if sign:
        team=os.environ.get('TEAM')
        if not team: raise RuntimeError('签名构建需要 TEAM 环境变量')
        args+=['-allowProvisioningUpdates',f'DEVELOPMENT_TEAM={team}']
        if device: args+=['-allowProvisioningDeviceRegistration']
    else: args+=['CODE_SIGNING_ALLOWED=NO']
    args += ['PRODUCT_BUNDLE_IDENTIFIER='+os.environ.get('BUNDLE_ID','dev.local.rhythm')]
    run(*args)
    app=ROOT/'target/ios-xcode/Build/Products/Release-iphoneos/Rhythm.app'
    if not sign: run('codesign','--force','--sign','-','--entitlements','ios/Rhythm.entitlements','--generate-entitlement-der',app)
    run('codesign','--verify',app)
    BUILD.mkdir(exist_ok=True)
    ipa=BUILD/'Rhythm.ipa'
    with zipfile.ZipFile(ipa,'w',compression=zipfile.ZIP_DEFLATED) as z:
        for p in app.rglob('*'):
            if p.is_file(): z.write(p,Path('Payload/Rhythm.app')/p.relative_to(app))
    print(ipa)

def smoke(seconds):
    prepare();BUILD.mkdir(exist_ok=True)
    env=os.environ.copy();env['NX_PHONE_PADS']='0'
    with (BUILD/'smoke.log').open('w') as out:
        p=subprocess.Popen([TOOL/'nxhost'],stdout=out,stderr=out,env=env)
        try: rc=p.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            p.terminate();p.wait(timeout=5)
            print(f'进程持续运行 {seconds} 秒；请结合画面和日志判断，不能据此认定可玩。');return
    print((BUILD/'smoke.log').read_text());sys.exit(rc if rc>0 else 1)

def main():
    parser=argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest='command',required=True)
    sub.add_parser('configure').add_argument('folder')
    for name in ['prepare','macos','test']: sub.add_parser(name)
    sh=sub.add_parser('shaders');sh.add_argument('sdk',choices=['macosx','iphoneos'])
    ip=sub.add_parser('ios');ip.add_argument('--signed',action='store_true');ip.add_argument('--device',help='已授权测试设备的 UDID，用于开发签名')
    sm=sub.add_parser('smoke');sm.add_argument('--seconds',type=int,default=20)
    args=parser.parse_args()
    if args.command=='configure':configure(args.folder)
    elif args.command=='prepare':prepare()
    elif args.command=='macos':macos()
    elif args.command=='ios':ios(args.signed,args.device)
    elif args.command=='shaders':shaders(args.sdk)
    elif args.command=='smoke':smoke(args.seconds)
    elif args.command=='test':run('cargo','test','--offline','-p','nxfs','-p','nxhost','-p','nxshader')

if __name__=='__main__':
    try: main()
    except (RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f'错误：{error}',file=sys.stderr);sys.exit(1)
