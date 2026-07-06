# Basic V1 iCloud Directory Mounts For CPSL

## Summary

- The PR branch is docs-only; the main issue is that `docs/icloud-drive-read-only-mounting-v1-plan.md` conflicts with the target `ro`/`rw` per-mount behavior.
- V1 will mount Apple-host-staged iCloud directory copies into CPSL. It will not implement live File Provider proxying, direct iCloud mutation, remembered bookmarks, or write-back.
- Three subagent review passes agreed: `rw` means writable staged local copy only, never sync back to iCloud.

## Key Changes

- Add a Herm-side `AuthorizedMount` or `CPSLMountDescriptor` contract with `label`, staged `host_path`, `/icloud/<slug>` `virtual_path`, `mode: ro|rw`, `source_platform`, `source_kind`, `access_lifetime`, `hydration_state`, and `writable_staged_copy`.
- Validate mounts before worker startup: canonical existing staged dirs, outside `/workdir` and other writable mounts; reject duplicate or shadow paths, bad slugs, reserved paths, unsupported providers, and `rw` unless `writable_staged_copy=true`.
- Add public Herm `--cpsl-session-config <path>` and internal worker `--session-config <path>`, keep legacy `--workspace`, and generate deterministic FFI session JSON with `/workdir:rw` plus sorted `/icloud/*` mounts.
- Extend CPSL prompts through `PromptData` so main agents and subagents render the same sanitized mount table. Do not expose host paths, Apple paths, bookmarks, security-scoped URLs, or provider IDs.
- Disable provider-side tools and CPSL HTTP egress when any iCloud-origin mount is active, regardless of `--allow-domain *`.
- Update docs to supersede the older broad/live-mount wording and state that V1 supports `ro` and explicit staged-only `rw`.

## Apple Host Contract

- macOS, iOS, or iPadOS host selects iCloud Drive directories through native pickers, materializes files, file-coordinates copy, and stages content into app-controlled storage outside writable CPSL mounts.
- The host passes only sanitized descriptors to Herm/CPSL.
- Revoke cancels active work, closes or restarts the worker, removes descriptors, clears prompt context, releases platform access, and deletes staged data.

Descriptor file shape:

```json
{
  "workspace": "/optional/workdir/fallback",
  "mounts": [
    {
      "label": "Project",
      "host_path": "/app/staged/icloud/project",
      "virtual_path": "/icloud/project",
      "mode": "ro",
      "source_platform": "ipados",
      "source_kind": "icloud-drive-directory",
      "access_lifetime": "session",
      "hydration_state": "staged",
      "writable_staged_copy": false
    }
  ]
}
```

The public Herm flag validates and preloads sanitized prompt rows. The worker
re-reads and validates the descriptor file immediately before creating the CPSL
session.

## Test Plan

- Unit tests: descriptor validation, slug/path rejection, duplicate/shadow/overlap rejection, mode defaults, `rw` staged flag enforcement.
- Worker tests: `--session-config` startup, legacy `--workspace` compatibility, deterministic JSON, multiple mounts.
- CPSL tests: read/list `/icloud/<slug>`, `ro` write/delete/rename failures, `rw` staged write success, traversal and symlink escape denial.
- Prompt/policy tests: main and subagent mount tables match; no host paths in prompts/traces; server tools and HTTP are disabled with active iCloud mounts.
- Manual Apple tests: hydrated folder staging, placeholder/offline/conflict/cancel failure paths, package directory handling, revocation cleanup.

## Assumptions

- V1 is iCloud Drive only, not generic File Provider support.
- Remote CPSL upload is out of scope unless added behind separate explicit upload consent.
- Any real iCloud write-back or live read-write mount is a later feature with its own broker, review UI, conflict handling, and rollback design.
