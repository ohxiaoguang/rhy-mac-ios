//! CPU/manual audio renderer bridge. Protocol and locking strategy adapted
//! from nxnative dm/host/hle_audren.c (MIT; see docs/PROVENANCE.md).
//! The game's SDK handles decoding, mixing and effects; only DeviceSink is
//! copied to Core Audio. Layout offsets are verified for SDK cff2680f.
use crate::stubs::{self, Args};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{LazyLock, Mutex, OnceLock};

const WORK: &str = "_ZN2nn5audio30GetAudioRendererWorkBufferSizeERKNS0_22AudioRendererParameterE";
const OPEN: &str = "_ZN2nn5audio17OpenAudioRendererEPNS0_19AudioRendererHandleEPNS_2os11SystemEventERKNS0_22AudioRendererParameterEPvm";
const OPEN0: &str =
    "_ZN2nn5audio17OpenAudioRendererEPNS0_19AudioRendererHandleERKNS0_22AudioRendererParameterEPvm";
const CLOSE: &str = "_ZN2nn5audio18CloseAudioRendererENS0_19AudioRendererHandleE";
const UPDATE: &str = "_ZN2nn5audio6server17AudioRenderSystem6UpdateEPvmS3_mPKvm";
const EXEC: &str = "_ZN2nn5audio29ExecuteAudioRendererRenderingENS0_19AudioRendererHandleE";
const EXEC_SYS: &str = "_ZN2nn5audio6server17AudioRenderSystem29ExecuteAudioRendererRenderingEv";
const EXEC_IMPL: &str = "_ZN2nn5audio6server24AudioRendererManagerImpl17AudioRendererImpl29ExecuteAudioRendererRenderingEv";
const GEN: &str = "_ZN2nn5audio6server17AudioRenderSystem15GenerateCommandENS_4SpanISt4byteEE";
const CREATE_THREAD: &str = "_ZN2nn2os12CreateThreadEPNS0_10ThreadTypeEPFvPvES3_S3_mii";
const START_THREAD: &str = "_ZN2nn2os11StartThreadEPNS0_10ThreadTypeE";
const WAIT_THREAD: &str = "_ZN2nn2os10WaitThreadEPNS0_10ThreadTypeE";
const DESTROY_THREAD: &str = "_ZN2nn2os13DestroyThreadEPNS0_10ThreadTypeE";
const SLEEP: &str = "_ZN2nn2os11SleepThreadENS_8TimeSpanE";
const LOCK: &str = "_ZN2nn2os12SdkMutexType4LockEv";
const UNLOCK: &str = "_ZN2nn2os12SdkMutexType6UnlockEv";
const PMAX: &str = "_ZN2nn5audio3dsp20CommandListProcessor17SetProcessTimeMaxEj";
const SETUP: &str = "_ZN2nn5audio3dsp20CommandListProcessor5SetupEPKvm";
const PROCESS: &str = "_ZN2nn5audio3dsp20CommandListProcessor7ProcessEv";
static SYMBOLS: OnceLock<HashMap<&'static str, u64>> = OnceLock::new();
static OPENED: AtomicBool = AtomicBool::new(false);
static HANDLE: Mutex<[u64; 2]> = Mutex::new([0; 2]);
static PCM: LazyLock<Mutex<VecDeque<i16>>> = LazyLock::new(Default::default);
static OUTPUT: Mutex<Option<super::output::Output>> = Mutex::new(None);
static RENDER: Mutex<()> = Mutex::new(());
static RENDERS: AtomicU64 = AtomicU64::new(0);
static LAST: AtomicU64 = AtomicU64::new(u64::MAX);
static THREAD: AtomicU64 = AtomicU64::new(0);
static STACK: AtomicU64 = AtomicU64::new(0);

pub fn init(mods: &[crate::loader::Module]) {
    let names = [
        WORK,
        OPEN,
        OPEN0,
        CLOSE,
        UPDATE,
        EXEC,
        EXEC_SYS,
        EXEC_IMPL,
        GEN,
        CREATE_THREAD,
        START_THREAD,
        WAIT_THREAD,
        DESTROY_THREAD,
        SLEEP,
        LOCK,
        UNLOCK,
        PMAX,
        SETUP,
        PROCESS,
    ];
    let map = names
        .into_iter()
        .filter_map(|n| crate::loader::symbol(mods, n).map(|a| (n, a)))
        .collect();
    let _ = SYMBOLS.set(map);
}
fn call(name: &str, args: &[u64]) -> u64 {
    let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(SYMBOLS.get().unwrap()[name] as usize) };
    let mut a = [0; 8];
    a[..args.len()].copy_from_slice(args);
    f(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7])
}
fn param(addr: u64) -> [u8; 64] {
    let mut p = [0; 64];
    unsafe { std::ptr::copy_nonoverlapping(addr as *const u8, p.as_mut_ptr(), 64) };
    p[0x2c..0x30].copy_from_slice(&1u32.to_le_bytes());
    p[0x30..0x34].copy_from_slice(&1u32.to_le_bytes());
    p
}
fn work(a: &Args) -> u64 {
    let p = param(a.0[0]);
    let n = call(WORK, &[p.as_ptr() as u64]);
    eprintln!("[audio-sdk] work buffer {n:#x}");
    n
}
fn open0(a: &Args) -> u64 {
    let p = param(a.0[1]);
    call(OPEN0, &[a.0[0], p.as_ptr() as u64, a.0[2], a.0[3]])
}
fn open(a: &Args) -> u64 {
    let p = param(a.0[2]);
    let result = call(OPEN, &[a.0[0], a.0[1], p.as_ptr() as u64, a.0[3], a.0[4]]);
    eprintln!("[audio-sdk] CPU/manual open result {result:#x}");
    if result != 0 || OPENED.swap(true, Relaxed) {
        return result;
    }
    unsafe {
        *HANDLE.lock().unwrap() = *(a.0[0] as *const [u64; 2]);
    }
    let rate = u32::from_le_bytes(p[0..4].try_into().unwrap());
    let frames = u32::from_le_bytes(p[4..8].try_into().unwrap()) as usize;
    *OUTPUT.lock().unwrap() = Some(super::output::start(
        rate,
        frames,
        Box::new(|out| {
            let mut q = PCM.lock().unwrap();
            static CALLS: AtomicU64 = AtomicU64::new(0);
            static GAPS: AtomicU64 = AtomicU64::new(0);
            if q.len() < out.len() {
                GAPS.fetch_add(1, Relaxed);
            }
            let calls = CALLS.fetch_add(1, Relaxed) + 1;
            if calls % 2000 == 0 {
                eprintln!(
                    "[audio-sdk] output callbacks {calls}, underrun blocks {}, queued frames {}",
                    GAPS.load(Relaxed),
                    q.len() / 2
                );
            }
            for s in out {
                *s = q.pop_front().unwrap_or(0);
            }
        }),
    ));
    // SDK ThreadType and stack must outlive the rendering thread.
    let tt = Box::into_raw(Box::new([0u128; 256])) as u64;
    let stack = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            0x20000,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        )
    } as u64;
    let r = call(
        CREATE_THREAD,
        &[tt, render_main as *const () as u64, 0, stack, 0x20000, 9, 2],
    );
    eprintln!("[audio-sdk] rendering thread create {r:#x}");
    if r == 0 {
        THREAD.store(tt, Relaxed);
        STACK.store(stack, Relaxed);
        call(START_THREAD, &[tt]);
    }
    result
}
extern "C" fn render_main(_: u64) {
    eprintln!("[audio-sdk] rendering thread started");
    while OPENED.load(Relaxed) {
        if PCM.lock().unwrap().len() < 1920 {
            let h = *HANDLE.lock().unwrap();
            call(EXEC, &h);
        } else {
            call(SLEEP, &[1_000_000]);
        }
    }
}
fn close(a: &Args) -> u64 {
    OPENED.store(false, Relaxed);
    let tt = THREAD.swap(0, Relaxed);
    if tt != 0 {
        call(WAIT_THREAD, &[tt]);
        call(DESTROY_THREAD, &[tt]);
        unsafe {
            drop(Box::from_raw(tt as *mut [u128; 256]));
            libc::munmap(STACK.swap(0, Relaxed) as *mut _, 0x20000);
        }
    }
    OUTPUT.lock().unwrap().take();
    PCM.lock().unwrap().clear();
    LAST.store(u64::MAX, Relaxed);
    call(CLOSE, &a.0[..2])
}
#[repr(C)]
struct Span {
    ptr: u64,
    len: u64,
}
unsafe fn read<T: Copy>(a: u64) -> T {
    unsafe { (a as *const T).read_unaligned() }
}
fn render(sys: u64) -> u64 {
    if RENDERS.load(Relaxed) == 0 {
        eprintln!("[audio-sdk] first render system {sys:#x}");
    }
    let _guard = crate::kernel::blocking(|| RENDER.lock().unwrap());
    call(LOCK, &[sys + 0x2d0]);
    let f: extern "C" fn(u64, u64, u64) -> Span =
        unsafe { std::mem::transmute(SYMBOLS.get().unwrap()[GEN] as usize) };
    let sp = unsafe { f(sys, read(sys + 0xf8), read(sys + 0x100)) };
    if RENDERS.load(Relaxed) == 0 {
        eprintln!("[audio-sdk] command list {:#x}+{:#x}", sp.ptr, sp.len);
    }
    call(UNLOCK, &[sys + 0x2d0]);
    let mut proc = [0u128; 16];
    let pp = proc.as_mut_ptr() as u64;
    call(PMAX, &[pp, u32::MAX as u64]);
    call(SETUP, &[pp, sp.ptr, sp.len]);
    call(PROCESS, &[pp]);
    if !capture(sp.ptr, sp.len) {
        // The stopped renderer can omit its sink. Still advance at audio
        // rate instead of consuming the game's voices in a tight loop.
        PCM.lock().unwrap().extend([0; 480]);
    }
    RENDERS.fetch_add(1, Relaxed);
    0
}
fn capture(cl: u64, len: u64) -> bool {
    unsafe {
        if RENDERS.load(Relaxed) < 2 && std::env::var_os("NX_AUDIO_DUMP_COMMANDS").is_some() {
            let _=std::fs::create_dir_all("work/audio");
            let _ = std::fs::write(
                format!("work/audio/commands-{}.bin", RENDERS.load(Relaxed)),
                std::slice::from_raw_parts(cl as *const u8, len as usize),
            );
        }
        let samples = read::<u32>(cl + 0x14) as usize;
        let mix = read::<u64>(cl + 8);
        let ncmd = read::<u32>(cl + 4);
        if RENDERS.load(Relaxed) < 2 {
            eprintln!("[audio-sdk] list commands {ncmd} samples {samples} bytes {len}");
        }
        if samples == 0 || samples > 1024 {
            return false;
        }
        let mut c = cl + 0x20;
        let trace = RENDERS.load(Relaxed) == 1;
        for _ in 0..ncmd {
            // SDK 21 removed the four-byte magic and compacted the
            // command header: enable +0, ID +1, size +2. Verified in
            // CommandListProcessor::Process (sdk+24fd50), DeviceSink
            // generation (sdk+23a7b0); its payload moved back eight bytes.
            if c + 8 > cl + len {
                break;
            }
            let size = read::<u16>(c + 2) as u64;
            if size < 8 || c + size > cl + len {
                break;
            }
            if trace {
                eprintln!(
                    "[audio-sdk] command {} size {size:#x} on {}",
                    read::<u8>(c + 1),
                    read::<u8>(c)
                );
            }
            if size == 0x140 && read::<u8>(c) & 1 != 0 && read::<u8>(c + 1) == 0x14 {
                let count = read::<u32>(c + 0x128) as usize;
                if !(1..=6).contains(&count) {
                    c += size;
                    continue;
                }
                let mut channels = [0u64; 6];
                for (i, p) in channels.iter_mut().take(count).enumerate() {
                    *p = mix + read::<u16>(c + 0x12c + 2 * i as u64) as u64 * samples as u64 * 4;
                }
                let mut out = Vec::with_capacity(samples * 2);
                let mut peak = 0i32;
                for i in 0..samples {
                    let v = |j: usize| read::<i32>(channels[j] + 4 * i as u64) as f64;
                    let (l, r) = if count == 6 {
                        (
                            v(0) + 0.707 * v(2) + 0.251 * v(3) + 0.707 * v(4),
                            v(1) + 0.707 * v(2) + 0.251 * v(3) + 0.707 * v(5),
                        )
                    } else {
                        (v(0), v(if count > 1 { 1 } else { 0 }))
                    };
                    for x in [l, r] {
                        let s = x.clamp(-32768., 32767.) as i16;
                        peak = peak.max((s as i32).abs());
                        out.push(s);
                    }
                }
                static LOGGED: AtomicBool = AtomicBool::new(false);
                if peak > 0 && !LOGGED.swap(true, Relaxed) {
                    eprintln!(
                        "[audio-sdk] non-silent output: {samples} frames, {count} inputs, peak {peak}"
                    );
                }
                PCM.lock().unwrap().extend(out);
                return true;
            }
            c += size;
        }
    }
    false
}
fn update(a: &Args) -> u64 {
    let n = RENDERS.load(Relaxed);
    let last = LAST.swap(n, Relaxed);
    if last != u64::MAX && last == n {
        render(a.0[0]);
    }
    let result = call(UPDATE, &a.0[..7]);
    LAST.store(RENDERS.load(Relaxed), Relaxed);
    result
}
pub fn overrides() -> Vec<(&'static str, u64)> {
    if std::env::var_os("NX_LEGACY_AUDIO").is_some() {
        return vec![];
    }
    [
        (WORK, work as fn(&Args) -> u64),
        (OPEN, open),
        (OPEN0, open0),
        (CLOSE, close),
        (UPDATE, update),
        (EXEC_SYS, |a: &Args| render(a.0[0])),
        (EXEC_IMPL, |a: &Args| render(a.0[0] + 0x60)),
    ]
    .into_iter()
    .map(|(name, f)| {
        (
            name,
            stubs::make(
                |tag, a| {
                    let f: fn(&Args) -> u64 = unsafe { std::mem::transmute(tag as usize) };
                    f(a)
                },
                f as usize as u64,
            ),
        )
    })
    .collect()
}
