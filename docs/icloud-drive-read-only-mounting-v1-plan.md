# Read-Only iCloud Directory Mounting V1 For iOS, iPadOS, And macOS

## Summary

- Three subagents agreed on this replacement plan: v1 covers **macOS, iOS, and iPadOS** with the same staged snapshot contract.
- The Apple host selects an iCloud Drive directory, hydrates/materializes it, copies it into app-controlled staging outside writable CPSL mounts, then Herm/CPSL mounts only that staged tree read-only at `/icloud/<slug>`.
- No platform gets a live provider-backed filesystem mount in v1. iOS/iPadOS uses Files/document picker selection, and macOS uses `NSOpenPanel` or equivalent user-selected folder access.

## Key Changes

- Add one shared `AuthorizedMount` descriptor:

```go
type AuthorizedMount struct {
    ScopeID        string
    Label          string
    HostPath       string // app-controlled staged directory visible to CPSL
    VirtualPath    string // /icloud/<slug>
    Mode           string // v1: ro
    SourcePlatform string // macos | ios | ipados
    SourceKind     string // icloud-drive-directory
    AccessLifetime string // session
    HydrationState string // staged
}
```

- Herm/CPSL stays platform-neutral: generate deterministic session JSON with `/workdir` as `rw` plus sorted `/icloud/*` mounts as `ro`.
- Add worker `--session-config <path>` for the new JSON startup path; keep current `--workspace` compatibility.
- Add one shared validator: canonical existing staged host directory, outside `/workdir` and other writable mounts; `/icloud/<slug>` only; reject duplicate/shadow/reserved/traversal paths; reject non-`ro`, non-session, non-staged, non-iCloud descriptors.
- Keep Apple credentials, bookmarks, security-scoped URLs, raw Apple paths, provider IDs, tokens, and raw host paths out of CPSL prompts, traces, logs, and session JSON.

## Platform Behavior

- macOS: use explicit folder selection only; no raw path entry, no broad iCloud scanning, no remembered bookmarks in v1.
- iOS/iPadOS: use Files/document picker directory selection, treat returned external URLs as security-scoped provider URLs, and open access only while actively staging. Apple's document-picker guidance requires security-scoped access and file coordination for outside-sandbox open access.
- V1 is **iCloud Drive only**. If the picker returns On My device, external storage, file server, or a third-party File Provider, reject with `unsupported_provider`. Future generic provider support should use `/files/<slug>`, not `/icloud/<slug>`.
- Bridge staging must file-coordinate reads/copies, check materialization per file, fail closed on placeholders/offline/provider errors/conflicts/cancellation, reject symlinks and Finder aliases, copy hardlinked files as independent files, and copy package directories read-only with package metadata in the manifest.
- Remote CPSL is not implicit. If any platform uses a remote CPSL session, require separate explicit upload consent and send only staged bytes plus sanitized manifest metadata.

## Prompt, Policy, And Lifecycle

- Render the same mount table for main agents and subagents:

```text
Available mounts:
- /workdir: writable workspace
- /icloud/project: read-only iCloud Drive snapshot, label "Project", platform iPadOS
```

- Prompt rules: read from `/icloud/<slug>`, write to `/workdir`, treat iCloud content as personal data.
- If any iCloud mount is active, force CPSL HTTP allow list to empty and disable provider-side tools such as `web_search` unless a later explicit exfiltration approval model exists.
- Mount changes restart the worker/session. Use lifecycle states: `selected`, `hydrating`, `staged`, `active`, `revoking`, `revoked`, `failed`.
- Revoke cancels active work, closes the worker, removes descriptors, clears prompt context, asks the platform host to release security scope, and deletes staged data.

## Test Plan

- Shared unit tests: descriptor validation, platform/source allowlists, slug/path rejection, duplicate/shadow rejection, host canonicalization and overlap rejection, `ro` enforcement.
- Worker/session tests: `--session-config` works; legacy `--workspace` still works; JSON contains `/workdir` `rw` plus `/icloud/project` `ro`.
- CPSL tests: read/list under `/icloud/project` succeeds; write/delete/rename/chmod/deep-create fail; `/workdir` remains writable; traversal and symlink escape fail.
- Prompt/policy tests: main and subagent prompts include the mount table and no host paths; server tools are disabled; CPSL HTTP is denied with active iCloud mounts.
- macOS integration/manual tests: selected hydrated iCloud folder stages and mounts; placeholder/offline/provider failure blocks mount; package directory copies read-only; symlink/alias rejection; revocation clears mount state.
- iOS/iPadOS real-device tests: directory picker iCloud Drive selection; security-scoped access is released after staging; app suspension/cancellation during staging leaves no partial mount; unsupported providers are rejected; remote upload requires separate consent if supported.

## Assumptions

- No Apple host app code exists in this repo yet, so this plan defines the host-bridge contract and Herm/CPSL plumbing.
- V1 excludes live read-only proxying, read-write mounts, `/icloud-output`, write-back, remembered bookmarks, hot mount/unmount, generic File Provider support, and background live filesystem service.
- CPSL core/FFI multi-mount support is reused as-is unless validation tests reveal a duplicate or shadowing hole that must be tightened.
