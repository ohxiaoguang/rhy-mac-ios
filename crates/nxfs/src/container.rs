//! Bounded PFS0/HFS0 reader. XCI opens the root HFS0's secure partition in place.
use crate::storage::{DynStorage, Sub};
use anyhow::{ensure, Context, Result};
use std::sync::Arc;

pub struct Entry { pub name: String, pub offset: u64, pub size: u64 }
pub struct Partition { storage: DynStorage, pub entries: Vec<Entry> }
impl Partition {
    pub fn open(storage: DynStorage) -> Result<Self> { Self::parse(storage, false) }
    fn parse(storage: DynStorage, trimmed_xci_root: bool) -> Result<Self> {
        let h = storage.read_vec(0, 16)?;
        let stride = match &h[..4] { b"PFS0" => 24, b"HFS0" => 64, _ => anyhow::bail!("expected PFS0 or HFS0") };
        let count = u32::from_le_bytes(h[4..8].try_into()?) as usize;
        let strings = u32::from_le_bytes(h[8..12].try_into()?) as usize;
        ensure!(count <= 65536 && strings <= 16 * 1024 * 1024, "invalid partition table size");
        let bytes = count * stride + strings;
        ensure!(16 + bytes as u64 <= storage.size(), "truncated partition table");
        let t = storage.read_vec(16, bytes)?;
        let names = &t[count * stride..];
        let data = 16 + bytes as u64;
        let mut entries = Vec::new();
        for i in 0..count {
            let e = &t[i * stride..];
            let relative = u64::from_le_bytes(e[..8].try_into()?);
            let mut size = u64::from_le_bytes(e[8..16].try_into()?);
            let so = u32::from_le_bytes(e[16..20].try_into()?) as usize;
            ensure!(so < names.len(), "invalid filename offset");
            let end = names[so..].iter().position(|&c| c == 0).context("unterminated filename")?;
            let name = std::str::from_utf8(&names[so..so+end])?.to_owned();
            ensure!(!name.is_empty() && !name.contains(['/', '\\']) && name != "..", "unsafe partition filename");
            let offset = data.checked_add(relative).context("partition offset overflow")?;
            // Some trimmed dumps omit the secure partition's final alignment sector.
            // Only the outer secure range can be clipped; inner NCA entries stay strict.
            if trimmed_xci_root && name == "secure" && offset <= storage.size() && size > storage.size() - offset {
                ensure!(size - (storage.size() - offset) <= 0x200, "truncated secure partition");
                size = storage.size() - offset;
            }
            ensure!(offset <= storage.size() && size <= storage.size() - offset, "partition entry outside file: {name}");
            entries.push(Entry { name, offset, size });
        }
        Ok(Self { storage, entries })
    }
    pub fn storage_of(&self, e: &Entry) -> DynStorage { Arc::new(Sub::new(self.storage.clone(), e.offset, e.size)) }
    pub fn file(&self, name: &str) -> Option<DynStorage> { self.entries.iter().find(|e| e.name == name).map(|e| self.storage_of(e)) }
}
pub fn open(storage: DynStorage) -> Result<Partition> {
    if storage.read_vec(0, 4)? == b"PFS0" { return Partition::open(storage); }
    ensure!(storage.size() >= 0x200, "truncated XCI header");
    let h = storage.read_vec(0x100, 0x40)?;
    ensure!(&h[..4] == b"HEAD", "expected NSP (PFS0) or XCI (HEAD)");
    let offset = u64::from_le_bytes(h[0x30..0x38].try_into()?);
    ensure!(offset < storage.size(), "invalid root HFS0 offset");
    let size = storage.size() - offset;
    let root = Partition::parse(Arc::new(Sub::new(storage, offset, size)), true)?;
    Partition::open(root.file("secure").context("XCI has no secure partition")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Mem;
    fn partition(magic: &[u8;4], name: &str, payload: &[u8]) -> Vec<u8> {
        let stride = if magic == b"PFS0" {24} else {64};
        let mut b = vec![0; 16 + stride + name.len() + 1];
        b[..4].copy_from_slice(magic); b[4..8].copy_from_slice(&1u32.to_le_bytes());
        b[8..12].copy_from_slice(&((name.len()+1) as u32).to_le_bytes());
        b[24..32].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        b[16+stride..16+stride+name.len()].copy_from_slice(name.as_bytes()); b.extend(payload); b
    }
    #[test] fn nsp_and_xci() {
        let nsp = partition(b"PFS0", "test.nca", b"payload");
        let p = open(Arc::new(Mem(nsp))).unwrap();
        assert_eq!(p.file("test.nca").unwrap().read_vec(0, 7).unwrap(), b"payload");
        let secure = partition(b"HFS0", "test.nca", b"payload");
        let root = partition(b"HFS0", "secure", &secure);
        let mut xci = vec![0; 0x200]; xci[0x100..0x104].copy_from_slice(b"HEAD");
        xci[0x130..0x138].copy_from_slice(&0x200u64.to_le_bytes()); xci.extend(root);
        let p = open(Arc::new(Mem(xci))).unwrap();
        assert_eq!(p.file("test.nca").unwrap().read_vec(0, 7).unwrap(), b"payload");
    }
    #[test] fn reject_malformed() {
        for b in [vec![0;4], partition(b"HFS0", "../bad", b"x"), {
            let mut b = partition(b"PFS0", "x", b"x"); b[16..24].copy_from_slice(&u64::MAX.to_le_bytes()); b
        }] { assert!(open(Arc::new(Mem(b))).is_err()); }
    }
}
