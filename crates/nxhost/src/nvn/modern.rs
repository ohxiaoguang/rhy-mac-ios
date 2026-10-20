//! ABI adapter for SDK cff2680f. Rendering uses the older canonical builder
//! layout; only the API boundary reads the newer SDK's packed objects.
use super::DRIVER;

pub fn active() -> bool { DRIVER.get().is_some_and(|d| d.modern) }
fn call(name: &str, args: &[u64]) -> u64 {
    let address = DRIVER.get().unwrap().procs[name];
    let f: extern "C" fn(u64,u64,u64,u64,u64,u64,u64,u64)->u64 = unsafe {std::mem::transmute(address as usize)};
    let mut a=[0;8];a[..args.len()].copy_from_slice(args);
    f(a[0],a[1],a[2],a[3],a[4],a[5],a[6],a[7])
}
fn float(name: &str, builder:u64)->f32 {
    let address=DRIVER.get().unwrap().procs[name];
    let f:extern "C" fn(u64)->f32=unsafe{std::mem::transmute(address as usize)};
    f(builder)
}

pub fn check_tiles(b:&[u8;0x68], bw:u32,bh:u32,bpb:u32,expected:(u32,u32)) {
    if !active() || std::env::var_os("NX_CHECK_LAYOUT").is_none() {return;}
    let u=|o|u32::from_le_bytes(b[o..o+4].try_into().unwrap()) as u64;
    let base=DRIVER.get().unwrap().property_helper;
    let setup:extern "C" fn(u64,u64,u64,u64,u64,u64,u64,u64,u64,u64,u64,u64)=unsafe{std::mem::transmute((base+0x2e0) as usize)};
    let pick:extern "C" fn(u64,u64)=unsafe{std::mem::transmute((base+0x3a0) as usize)};
    let mut desc=[0u64;8];let mut tiles=[0u8;8];
    setup(desc.as_mut_ptr() as u64,u(0x18),u(0x1c),bpb as u64,u(0x34),bw as u64,bh as u64,u(0x2c),u(0x20),u(0x24),u(0x28),1);
    pick(desc.as_ptr() as u64,tiles.as_mut_ptr() as u64);
    if (tiles[1] as u32,tiles[2] as u32)!=expected {
        eprintln!("[layout] {}x{} fmt{} flags{:x}: host {:?}, SDK {:?}",u(0x20),u(0x24),u(0x30),u(0x18),expected,&tiles[..3]);
    }
}
pub fn texture(builder: u64) -> [u8;0x68] {
    let src=unsafe{std::slice::from_raw_parts(builder as *const u8,0x68)};
    let mut b=[0;0x68];
    if !active() {b.copy_from_slice(src);return b;}
    b[0..0x18].copy_from_slice(&src[8..0x20]);
    b[0x18..0x4c].copy_from_slice(&src[0x20..0x54]);
    b[0x50..0x58].copy_from_slice(&src[0x60..0x68]);
    b[0x58..0x5a].copy_from_slice(&src[0x54..0x56]);
    b[0x5c..0x60].copy_from_slice(&src[0x58..0x5c]);
    b
}
pub fn sampler(builder:u64)->[u8;0x60] {
    let mut b=[0;0x60];
    if !active() {unsafe{std::ptr::copy_nonoverlapping(builder as *const u8,b.as_mut_ptr(),b.len())};return b;}
    let p=b.as_mut_ptr() as u64;
    call("nvnSamplerBuilderGetMinMagFilter",&[builder,p+8,p+0xc]);
    call("nvnSamplerBuilderGetWrapMode",&[builder,p+0x10,p+0x14,p+0x18]);
    call("nvnSamplerBuilderGetLodClamp",&[builder,p+0x1c,p+0x20]);
    b[0x24..0x28].copy_from_slice(&float("nvnSamplerBuilderGetLodBias",builder).to_le_bytes());
    call("nvnSamplerBuilderGetCompare",&[builder,p+0x28,p+0x2c]);
    call("nvnSamplerBuilderGetBorderColor",&[builder,p+0x30]);
    b[0x40..0x44].copy_from_slice(&float("nvnSamplerBuilderGetMaxAnisotropy",builder).to_le_bytes());
    b
}
pub fn view(view:u64)->[u8;0x28] {
    let mut b=[0;0x28];let p=b.as_mut_ptr() as u64;
    let mut base=0i32;let mut count=0i32;
    if call("nvnTextureViewGetLevels",&[view,&mut base as *mut _ as u64,&mut count as *mut _ as u64])!=0 {
        b[0]|=1;b[1]=base as u8;
    }
    if call("nvnTextureViewGetLayers",&[view,&mut base as *mut _ as u64,&mut count as *mut _ as u64])!=0 {
        b[0]|=2;b[4..6].copy_from_slice(&(base as u16).to_le_bytes());
    }
    if call("nvnTextureViewGetFormat",&[view,p+8])!=0 {b[0]|=4;}
    if call("nvnTextureViewGetSwizzle",&[view,p+0xc,p+0x10,p+0x14,p+0x18])!=0 {b[0]|=8;}
    if call("nvnTextureViewGetDepthStencilMode",&[view,p+0x1c])!=0 {b[0]|=0x10;}
    if call("nvnTextureViewGetTarget",&[view,p+0x20])!=0 {b[0]|=0x20;}
    b
}
