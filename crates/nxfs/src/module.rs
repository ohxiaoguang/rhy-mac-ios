// Modified for rhy-mac-ios on 2026-10-20; see NOTICE and docs/PROVENANCE.md.
// Mush8 - module.rs
// Constantin Clerc (c22dev) - 0.1
use anyhow::{Result, bail, ensure};

pub const R_AARCH64_ABS64: u32 = 257;
pub const R_AARCH64_GLOB_DAT: u32 = 1025;
pub const R_AARCH64_JUMP_SLOT: u32 = 1026;
pub const R_AARCH64_RELATIVE: u32 = 1027;

#[derive(Debug, Clone, Copy)]
pub struct Rela {
    pub offset: u64,
    pub kind: u32,
    pub sym: u32,
    pub addend: i64,
}

#[derive(Debug, Clone)]
pub struct Sym {
    pub name: String,
    pub value: u64,
    pub info: u8,
    pub shndx: u16,
}

impl Sym {
    pub fn defined(&self) -> bool {
        self.shndx != 0
    }
    pub fn weak(&self) -> bool {
        self.info >> 4 == 2
    }
}

#[derive(Debug, Default)]
pub struct Dynamic {
    pub mod0: u64,
    pub bss: (u64, u64),
    pub relocs: Vec<Rela>,
    pub symbols: Vec<Sym>,
    pub init: Option<u64>,
    pub init_array: Option<(u64, u64)>,
    pub fini_array: Option<(u64, u64)>,
}

fn u32_at(b: &[u8], o: u64) -> Result<u32> {
    let o = o as usize;
    ensure!(o + 4 <= b.len(), "read past image at {o:#x}");
    Ok(u32::from_le_bytes(b[o..o + 4].try_into()?))
}
fn u64_at(b: &[u8], o: u64) -> Result<u64> {
    let o = o as usize;
    ensure!(o + 8 <= b.len(), "read past image at {o:#x}");
    Ok(u64::from_le_bytes(b[o..o + 8].try_into()?))
}

pub fn parse(image: &[u8]) -> Result<Dynamic> {
    let mod0 = u32_at(image, 4)? as u64;
    if u32_at(image, mod0)? != u32::from_le_bytes(*b"MOD0") {
        bail!("no MOD0 header");
    }
    let rel = |o: u64| -> Result<u64> { Ok((mod0 as i64 + u32_at(image, mod0 + o)? as i32 as i64) as u64) };
    let dynamic = rel(4)?;
    let mut d = Dynamic { mod0, bss: (rel(8)?, rel(12)?), ..Default::default() };

    let (mut rela, mut relasz, mut jmprel, mut pltrelsz) = (0, 0, 0, 0);
    let (mut relr, mut relrsz, mut relrent) = (0, 0, 8);
    let (mut symtab, mut strtab, mut syment, mut hash, mut gnu_hash) = (0, 0, 24, 0, 0);
    let (mut init_array, mut init_arraysz, mut fini_array, mut fini_arraysz) = (None, 0, None, 0);
    let mut o = dynamic;
    loop {
        let (tag, val) = (u64_at(image, o)? as i64, u64_at(image, o + 8)?);
        o += 16;
        match tag {
            0 => break,
            2 => pltrelsz = val,
            4 => hash = val,
            0x6ffffef5 => gnu_hash = val,
            5 => strtab = val,
            6 => symtab = val,
            7 => rela = val,
            8 => relasz = val,
            11 => syment = val,
            12 => d.init = Some(val),
            23 => jmprel = val,
            25 => init_array = Some(val),
            26 => fini_array = Some(val),
            27 => init_arraysz = val,
            28 => fini_arraysz = val,
            35 => relrsz = val,
            36 => relr = val,
            37 => relrent = val,
            _ => {}
        }
    }
    d.init_array = init_array.map(|a| (a, init_arraysz));
    d.fini_array = fini_array.map(|a| (a, fini_arraysz));

    for (start, size) in [(rela, relasz), (jmprel, pltrelsz)] {
        let mut r = start;
        while r < start + size {
            let info = u64_at(image, r + 8)?;
            d.relocs.push(Rela {
                offset: u64_at(image, r)?,
                kind: info as u32,
                sym: (info >> 32) as u32,
                addend: u64_at(image, r + 16)? as i64,
            });
            r += 24;
        }
    }

    ensure!(relrent == 8 && relrsz % 8 == 0, "invalid RELR entry size");
    decode_relr(image, relr, relrsz, &mut d.relocs)?;
    let nsyms = if hash != 0 { u32_at(image, hash + 4)? as u64 }
    else {
        ensure!(gnu_hash != 0, "no ELF symbol hash table");
        gnu_symbol_count(image, gnu_hash)?
    };
    ensure!(syment == 24 && symtab <= image.len() as u64 && nsyms <= (image.len() as u64 - symtab) / syment, "invalid symbol table");
    for i in 0..nsyms {
        let s = symtab + i * syment;
        let name_off = strtab + u32_at(image, s)? as u64;
        ensure!(name_off < image.len() as u64, "symbol name outside image");
        let end = image[name_off as usize..].iter().position(|&c| c == 0).unwrap_or(0);
        d.symbols.push(Sym {
            name: String::from_utf8_lossy(&image[name_off as usize..name_off as usize + end]).into_owned(),
            info: image[s as usize + 4],
            shndx: u16::from_le_bytes([image[s as usize + 6], image[s as usize + 7]]),
            value: u64_at(image, s + 8)?,
        });
    }
    Ok(d)
}

// ELF64 RELR: an even word relocates one address; an odd word describes the
// following 63 pointer slots. Implicit addends must be captured before rebasing.
fn decode_relr(image: &[u8], start: u64, size: u64, out: &mut Vec<Rela>) -> Result<()> {
    let end = start.checked_add(size).ok_or_else(|| anyhow::anyhow!("RELR range overflow"))?;
    ensure!(end <= image.len() as u64 && size % 8 == 0, "RELR outside image");
    let mut next = None;
    let mut push = |offset| -> Result<()> {
        ensure!(offset % 8 == 0, "unaligned RELR target");
        out.push(Rela { offset, kind: R_AARCH64_RELATIVE, sym: 0, addend: u64_at(image, offset)? as i64 });
        Ok(())
    };
    for o in (start..end).step_by(8) {
        let word = u64_at(image, o)?;
        if word & 1 == 0 {
            push(word)?;
            next = Some(word.checked_add(8).ok_or_else(|| anyhow::anyhow!("RELR target overflow"))?);
        } else {
            let base = next.ok_or_else(|| anyhow::anyhow!("RELR bitmap before address"))?;
            for bit in 1..64 { if word & (1u64 << bit) != 0 { push(base + (bit-1)*8)?; } }
            next = Some(base.checked_add(63*8).ok_or_else(|| anyhow::anyhow!("RELR target overflow"))?);
        }
    }
    Ok(())
}
#[cfg(test)]
mod relr_tests {
    use super::*;
    #[test] fn address_and_bitmaps() {
        let mut image = vec![0; 2048];
        for (o, v) in [(0, 512u64), (8, 1 | (1<<1) | (1<<3)), (16, 1 | (1<<1)), (512, 100), (520, 200), (536, 300), (1024, 400)] {
            image[o..o+8].copy_from_slice(&v.to_le_bytes());
        }
        let mut out=vec![]; decode_relr(&image, 0, 24, &mut out).unwrap();
        assert_eq!(out.iter().map(|r| (r.offset,r.addend)).collect::<Vec<_>>(), vec![(512,100),(520,200),(536,300),(1024,400)]);
    }
    #[test] fn invalid_bitmap_and_target() {
        let mut b = vec![0;16]; b[..8].copy_from_slice(&3u64.to_le_bytes());
        assert!(decode_relr(&b,0,8,&mut vec![]).is_err());
        b[..8].copy_from_slice(&1024u64.to_le_bytes());
        assert!(decode_relr(&b,0,8,&mut vec![]).is_err());
    }
}

fn gnu_symbol_count(image: &[u8], table: u64) -> Result<u64> {
    let buckets = u32_at(image, table)? as u64;
    let first = u32_at(image, table+4)? as u64;
    let bloom = u32_at(image, table+8)? as u64;
    let bucket_base = table + 16 + bloom*8;
    let chains = bucket_base + buckets*4;
    ensure!(chains <= image.len() as u64, "GNU hash table outside image");
    let mut count = first;
    for i in 0..buckets {
        let mut symbol = u32_at(image, bucket_base + i*4)? as u64;
        if symbol == 0 { continue; }
        ensure!(symbol >= first, "invalid GNU hash bucket");
        loop {
            let value = u32_at(image, chains + (symbol-first)*4)?;
            symbol += 1;
            if value & 1 != 0 { break; }
        }
        count = count.max(symbol);
    }
    Ok(count)
}
