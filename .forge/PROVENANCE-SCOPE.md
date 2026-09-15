# Why `.forge/` is excluded from the provenance lint

`scripts/lint-provenance.py` has two jobs: (1) no third-party CDN hosts anywhere in
the repo, (2) no verbatim third-party product **copy** in strings **we ship**.

`.forge/` is added to that lint's `SKIP_DIRS` (alongside the pre-existing `provenance`
and `vendor` exclusions) for two reasons:

1. **It never ships.** `.forge/` is FORGE project-management scaffolding — planning
   docs and per-phase evidence. None of it is compiled into or bundled with the
   `Hickeyfield.app`. The copy-provenance check exists to protect *shipped* UI/source
   strings; `.forge/` has no shipped strings to protect.
2. **Evidence docs must quote the strings under discussion.** A test/review file that
   documents "the phrase X incidentally matched the corpus" necessarily contains X.
   Scanning evidence for corpus matches therefore produces perpetual false positives
   on the very act of recording the fix.

The CDN-host check still has full teeth on all shipped source, and the copy check still
scans every shipped `.rs/.ts/.tsx/.css/.html/.json/...` file. Only the non-shipped
`.forge/` tree is exempt.

Decision approved by the repo owner on 2026-09-14. This is FORGE-toolkit housekeeping,
independent of any single slice's production code.
