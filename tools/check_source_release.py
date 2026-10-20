#!/usr/bin/env python3
"""Check the Git index before a source-only publication. Never print secrets."""
# SPDX-License-Identifier: GPL-3.0-only
import re
import subprocess
import sys
from pathlib import PurePosixPath

BLOCKED_ROOTS = {"target", "work", "build", "Firmware"}
BLOCKED_SUFFIXES = {
    ".xci", ".nsp", ".nca", ".nso", ".nro", ".keys", ".key", ".bin",
    ".image", ".layout", ".metallib", ".air", ".ipa", ".zip", ".7z",
    ".mobileprovision", ".provisionprofile", ".p12", ".pfx", ".p8", ".pem",
    ".cer", ".bfttf", ".npdm", ".nacp", ".wav", ".mp3", ".mp4", ".webm",
    ".png", ".jpg", ".jpeg", ".webp", ".gif", ".pyc", ".log",
}
SECRET_PATTERNS = [
    re.compile(rb"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----"),
    re.compile(rb"(?im)^\s*(?:master_key_[0-9a-f]+|titlekek_[0-9a-f]+|"
               rb"key_area_key_[a-z_0-9]+|header_key|aes_kek_generation_source)"
               rb"\s*=\s*[0-9a-f]{32,}\s*$"),
    re.compile(rb"\bgh[pousr]_[A-Za-z0-9]{30,}\b"),
    re.compile(rb"\bgithub_pat_[A-Za-z0-9_]{30,}\b"),
]


def main():
    raw = subprocess.check_output(["git", "ls-files", "--stage", "-z"])
    errors, count = [], 0
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, oid, stage = metadata.decode("ascii").split()
        name = raw_path.decode("utf-8")
        path = PurePosixPath(name)
        count += 1
        if stage != "0" or mode not in {"100644", "100755"}:
            errors.append((name, "unmerged entry, symlink or non-regular source"))
            continue
        if (path.parts[0] in BLOCKED_ROOTS or path.parts[:2] == ("docs", "local")
                or path.suffix.lower() in BLOCKED_SUFFIXES
                or any(p.endswith((".app", ".xcresult", ".xcodeproj")) for p in path.parts)
                or path.name == ".DS_Store" or path.name.startswith(".env")):
            errors.append((name, "local/generated/resource path"))
            continue
        data = subprocess.check_output(["git", "cat-file", "blob", oid])
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            errors.append((name, "non-UTF-8 binary content"))
        if b"\0" in data or len(data) > 1024 * 1024:
            errors.append((name, "binary content or unexpectedly large source"))
        if any(pattern.search(data) for pattern in SECRET_PATTERNS):
            errors.append((name, "possible credential/key material (content hidden)"))
    if not count:
        print("FAIL: Git index is empty; stage the intended source files first.")
        return 1
    for name, reason in errors:
        print(f"FAIL: {name}: {reason}")
    if errors:
        return 1
    print(f"PASS: {count} indexed text source/configuration/license files; no blocked content found.")
    print("This check does not establish legal clearance or exhaustive secret detection.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
