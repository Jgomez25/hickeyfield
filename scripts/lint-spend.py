#!/usr/bin/env python3
"""Spend-safety lint. A CI gate that makes accidental spending structurally hard.

The 2026-09-16 incident (a unit test that reached the login keychain and
submitted three real generations for ~$0.36) showed that guarding the vault is
not enough on its own: any code that reads a raw credential, strips the dummy
env, or flips the spend switch can reopen the hole. This linter scans every
tracked Rust source and fails the build when one of those patterns appears
outside the small, named set of files that legitimately need it.

Four rules, each with a documented violating example (see the header of each
check below). All four are substring/regex hits; the allowlist is file-scoped,
so adding a new legitimate reader is deliberate, reviewable friction.

Usage:
    scripts/lint-spend.py            # scan every tracked .rs (the gate)
    scripts/lint-spend.py FILE...    # scan the given files instead (for
                                     # proving detection on a scratch file)
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Files that may legitimately read a raw provider credential.
#   - vault.rs           the single vault + its env-override/guard tests
#   - first_light.rs     the one documented FAL_KEY dev seam (a real generation)
#   - live_fal_pricing.rs the #[ignore] read-only pricing probe
KEY_READ_ALLOW = (
    "src-tauri/src/vault.rs",
    "crates/hickeyfield-core/examples/first_light.rs",
    "crates/hickeyfield-core/tests/live_fal_pricing.rs",
)
# Only the vault speaks to the OS keychain.
KEYRING_ALLOW = ("src-tauri/src/vault.rs",)
# The vault owns the `hickeyfield_*_KEY` / `FAL_KEY` env vars, and its
# keychain-guard tests (the 2026-09-16 incident's regression tests) must strip
# them to prove the guard. Nowhere else may unset a credential env var — that
# is the exact env-stripping pattern that caused the incident.
REMOVE_VAR_ALLOW = ("src-tauri/src/vault.rs",)
# Only the packaged app's Tauri entrypoint may enable spending.
ENABLE_ALLOW = ("src-tauri/src/lib.rs",)

# Rule 2: a raw credential *read* from the process environment.
#   e.g.  let k = std::env::var("FAL_KEY")?;   -> outside the allowlist, fail.
# Matches an `env::var`/`var_os` call on a credential-shaped name literal, not a
# bare mention: `onboarding.rs` legitimately lists "FAL_KEY" as a recognized
# env-name for the .env import feature and uses it as parse_env test data —
# neither reads the process env, so a bare-substring rule would false-positive.
RE_KEY_READ = re.compile(
    r'\bvar(?:_os)?\(\s*"(?:FAL_KEY|hickeyfield_[A-Z0-9_]*_KEY)"'
)
# Rule 1: any keychain access.
#   e.g.  keyring::Entry::new(...)   /   .get_password()
RE_KEYRING = re.compile(r"keyring::|get_password")
# Rule 3: unsetting a credential env var.
#   e.g.  std::env::remove_var("hickeyfield_FAL_KEY");
RE_REMOVE_VAR = re.compile(r'remove_var\("(?:hickeyfield_|FAL_KEY)')
# Rule 4a: a test (or anything) turning spending ON via the env — never allowed.
#   e.g.  std::env::set_var("HICKEYFIELD_ALLOW_SPEND", "1");
RE_SET_ALLOW_SPEND = re.compile(r'set_var\("HICKEYFIELD_ALLOW_SPEND')
# Rule 4b: calling the process-global enable() outside the entrypoint.
#   e.g.  hickeyfield_core::spend_gate::enable();   in a test -> fail.
RE_ENABLE = re.compile(r"spend_gate::enable\(")


def tracked_rs() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "*.rs"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [REPO / line for line in out.splitlines() if line]


def rel(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(REPO)).replace("\\", "/")
    except ValueError:
        return str(path)


def check_file(path: Path, violations: list[str]) -> None:
    relpath = rel(path)
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as e:  # pragma: no cover - unreadable file
        violations.append(f"{relpath}: could not read ({e})")
        return

    for n, line in enumerate(text.splitlines(), start=1):
        if RE_KEY_READ.search(line) and not relpath.endswith(KEY_READ_ALLOW):
            violations.append(
                f"{relpath}:{n} reads a raw provider credential "
                f"(FAL_KEY/hickeyfield_*_KEY) outside the vault — read it via vault::get"
            )
        if RE_KEYRING.search(line) and not relpath.endswith(KEYRING_ALLOW):
            violations.append(
                f"{relpath}:{n} touches the OS keychain (keyring::/get_password) "
                f"outside vault.rs"
            )
        if RE_REMOVE_VAR.search(line) and not relpath.endswith(REMOVE_VAR_ALLOW):
            violations.append(
                f"{relpath}:{n} unsets a credential env var — this is the "
                f"env-stripping pattern that caused the 2026-09-16 spend incident"
            )
        if RE_SET_ALLOW_SPEND.search(line):
            violations.append(
                f"{relpath}:{n} sets HICKEYFIELD_ALLOW_SPEND — a test must never "
                f"enable spending"
            )
        if RE_ENABLE.search(line) and not relpath.endswith(ENABLE_ALLOW):
            violations.append(
                f"{relpath}:{n} calls spend_gate::enable() outside the app "
                f"entrypoint (src-tauri/src/lib.rs)"
            )


def main(argv: list[str]) -> int:
    if argv:
        files = [Path(a) for a in argv]
    else:
        files = tracked_rs()

    violations: list[str] = []
    for path in files:
        check_file(path, violations)

    if violations:
        for v in violations:
            print(f"lint-spend: {v}", file=sys.stderr)
        print(f"\nlint-spend: {len(violations)} violation(s).", file=sys.stderr)
        return 1

    print(f"lint-spend: ok ({len(files)} file(s) scanned).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
