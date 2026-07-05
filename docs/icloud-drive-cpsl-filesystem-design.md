# iOS and macOS iCloud Drive Mounts in CPSL

Status: feasibility design

## Summary

Selected iCloud Drive directories can be exposed to a CPSL capsule on iOS,
iPadOS, and macOS when an Apple-platform host bridge mediates user consent,
local availability, revocation, and write-back. CPSL should receive only
already-authorized local directories or staged file trees. It must not receive
Apple Account credentials, iCloud tokens, security-scoped bookmark data, raw
iOS container paths, or a general entitlement to the user's iCloud Drive.

The target product shape is scoped directory mounting:

1. The user selects one or more iCloud Drive directories through a native Apple
   platform picker.
2. The host bridge records a session-only grant or remembered platform bookmark.
3. The host bridge verifies that selected files are locally available or can be
   materialized.
4. Herm or the Apple host starts a CPSL capsule with explicit mounts.
5. CPSL sees selected directories at stable virtual paths such as
   `/icloud/project`.
6. iCloud mounts are read-only by default.
7. Writes go to a CPSL workspace or staging mount and are applied back to
   iCloud only after host-side review, file coordination, and conflict checks.

This is not a direct Linux/server-side iCloud Drive filesystem. Apple supports
user-selected file and directory access on Apple platforms and local iCloud
Drive sync surfaces, not a public OAuth/WebDAV/REST API for arbitrary user
iCloud Drive mounting.

## Consensus

Three independent investigation passes agreed on these points:

- The design must explicitly cover both macOS and iOS/iPadOS. The old
  "Apple-device bridge" wording was too vague.
- macOS is the best first live-mount host because iCloud Drive appears as a
  local Finder/Open Panel surface and selected directories can be mounted or
  staged by a long-running host process.
- iOS/iPadOS support must be framed as a Files app/document-picker bridge. It
  can provide snapshots, live read-only proxying, or staged write-back, but it
  should not be described as a kernel/FUSE mount or a remote filesystem grant.
- CPSL core and FFI can represent multiple mounts and `ro`/`rw` mount modes.
  The current Herm CPSL worker path is narrower: it passes one host workspace
  mounted at `/workdir`.
- Scoped extra mounts are the right long-term architecture:
  `/workdir` for the normal writable workspace, `/icloud/<name>` for read-only
  iCloud directories, and an optional staging path for outputs.
- Mount lifecycle is static for the current CPSL session config. Adding or
  revoking an iCloud mount should restart the Herm CPSL worker/session until a
  hot-mount protocol exists.
- Main risks are data exfiltration, accidental broad scope, unhydrated
  placeholders, destructive writes syncing to other devices, iCloud conflicts,
  POSIX metadata mismatches, background execution limits on iOS, and leakage of
  platform authorization artifacts.

## Platform Model

### macOS

macOS is the primary v1 platform for live local iCloud directory mounts.

Use `NSOpenPanel` or equivalent SwiftUI file importer UI for explicit folder
selection. Configure folder selection rather than free path entry; do not
hard-code `~/Library/Mobile Documents/...` or scan broad iCloud Drive paths.
The host should operate on the URL returned by the picker or by resolving a
stored bookmark.

macOS host modes:

- `snapshot`: the bridge coordinates and copies selected content into
  app-controlled staging, then CPSL mounts the staged tree.
- `live-readonly`: CPSL receives a read-only mount of a selected local iCloud
  Drive directory after the bridge validates scope and hydration.
- `staged-writeback`: CPSL writes to `/workdir` or `/icloud-output/<name>`;
  the bridge reviews and applies approved changes back to iCloud Drive.

Direct live read-write mounts are out of scope for v1. They require a
single-writer rule, recursive hydration, file coordination at the broker layer,
stale-write detection, conflict UI, and clear rollback behavior.

### iOS and iPadOS

iOS/iPadOS support is a host-app bridge through Files and document picker APIs,
not a POSIX mount of the user's iCloud Drive.

Use `UIDocumentPickerViewController` for directory and file selection. For
directory scopes, use the system directory-picking flow; for file scopes, use
document picker document types. Treat selected locations as provider-backed
URLs that may refer to iCloud Drive, On My iPhone/iPad, external storage, file
servers, or third-party File Provider locations.

iOS/iPadOS host modes:

- `snapshot`: coordinate and copy selected files into app-controlled storage,
  then run an on-device CPSL capsule over that staged tree or upload the staged
  tree to a remote CPSL session.
- `live-readonly`: for on-device CPSL only, CPSL file operations are brokered
  through the host app, which opens security-scoped URLs per operation and
  releases them promptly.
- `staged-writeback`: CPSL writes to app-controlled staging; the host app shows
  a review queue and applies approved changes back through coordinated writes.

For remote CPSL sessions, never send security-scoped URLs, bookmark data, raw
Apple filesystem paths, iCloud identifiers, or provider-private identifiers.
Send only selected file bytes, selected metadata needed for review/conflict
checks, and virtual mount identifiers.

Long-running live proxying is not a reliable iOS background design. A remote
session should receive a packaged snapshot or app-controlled upload. Background
`URLSession` can support uploads/downloads, but the design must not depend on a
suspended iOS app keeping a live file proxy open.

## CPSL and Herm Constraints

CPSL itself is a sandboxed, Unix-like runtime with explicit modules, files,
mounts, and network rules. The core mount table enforces read-only mounts,
write checks, path traversal denial, and longest-prefix virtual path
resolution.

Current repo facts:

- CPSL core supports multiple mounts through `MountTable`.
- CPSL FFI session JSON contains a `mounts` array and accepts `"ro"` and
  `"rw"` modes.
- Standalone CPSL manifests support mount specs in `[mounts].volumes` using
  `host:virtual[:ro]`.
- Herm's current CPSL worker still accepts a single `--workspace` value and
  builds one `/workdir` mount from it.
- Herm's CPSL prompt profile currently assumes `/workdir` is the only project
  file root.

Relevant paths:

- `README.md`: CPSL is Unix-like with explicit modules, files, mounts, and
  network rules.
- `core/src/mount.rs`: mount table, read-only enforcement, and path resolution.
- `ffi/src/lib.rs`: FFI session config, multi-mount validation, and sandbox
  creation.
- `cli/src/config.rs` and `cli/src/main.rs`: manifest and CLI volume handling.
- `herm/cmd/herm/cpsl_worker.go`, `herm/cmd/herm/cpsl_client.go`, and
  `herm/cmd/herm/cpsl_library.go`: current Herm single-workspace worker path.
- `herm/cmd/herm/promptprofile.go` and `herm/prompts/cpsl/`: prompt text that
  currently orients the agent around `/workdir`.

## Mount Contract

Apple-platform code should return neutral mount descriptors to Herm/CPSL. CPSL
must not understand Apple credentials, document pickers, bookmarks, CloudKit,
or File Provider implementation details.

```json
{
  "scope_id": "macos-bookmark-uuid-or-ios-session-id",
  "label": "Project",
  "host_path": "/authorized/local/path/or/staged/tree",
  "virtual_path": "/icloud/project",
  "mode": "ro",
  "source_platform": "macos",
  "source_kind": "icloud-drive-directory",
  "access_lifetime": "session",
  "hydration_state": "downloaded",
  "freshness": {
    "snapshot_id": "optional",
    "created_at": "2026-07-04T00:00:00Z"
  }
}
```

Required validation:

- `virtual_path` must be absolute.
- iCloud mount paths must live under `/icloud/<name>`.
- `<name>` must be a stable slug.
- Reject empty names, `.`, `..`, `/`, path separators, duplicate names, and
  duplicate virtual paths.
- Reject mounts that shadow `/workdir`, `/tmp`, `/cache`, `/attachments`, or
  another iCloud mount.
- Reject host paths that cannot be canonicalized.
- Default iCloud mount mode is `ro`.
- A `rw` iCloud mount requires an explicit advanced mode and is out of scope
  for v1.

Target CPSL session shape:

```json
{
  "mounts": [
    {"host": "/path/to/workspace", "virtual": "/workdir", "mode": "rw"},
    {"host": "/authorized/staged/project", "virtual": "/icloud/project", "mode": "ro"},
    {"host": "/path/to/output", "virtual": "/icloud-output/project", "mode": "rw"}
  ],
  "initial_cwd": "/workdir",
  "language": "luau",
  "http": {
    "mode": "policy",
    "allow_domains": [],
    "deny_domains": []
  }
}
```

## Mount Lifecycle

Current CPSL session config is fixed at `cpsl_session_new`. Until Herm grows a
worker lifecycle operation for adding and removing mounts, any mount change
requires a worker/session restart.

Mount start:

1. User selects a directory.
2. Platform bridge creates or resolves the platform authorization grant.
3. Platform bridge validates and hydrates the selected tree.
4. Platform bridge returns one or more `AuthorizedMount` descriptors.
5. Herm restarts the CPSL worker with `/workdir` plus `/icloud/<name>` mounts.
6. Prompt context is rebuilt with the active mount table.

Mount revoke:

1. Stop new agent turns.
2. Close the CPSL worker/session.
3. Remove mount descriptors from the session config.
4. Clear prompt context and UI state.
5. Delete staged copies when appropriate.
6. Stop platform security-scoped access.
7. Delete remembered bookmark data when the user revokes persistent access.
8. Restart CPSL without the revoked mount if the app session continues.

The design should not imply hot-mount support until the worker protocol
supports a safe mount lifecycle API.

## macOS Bridge Requirements

### Authorization

Phase 1 should support session-only access. Remembered access is a separate
milestone using security-scoped bookmarks.

For a sandboxed Mac app:

- Use user-selected read-only entitlement for read-only exposure.
- Use user-selected read-write entitlement only when write-back is enabled.
- Do not assume a child CPSL worker inherits post-launch PowerBox access. Keep
  iCloud I/O inside the host bridge or pass only staged directories the worker
  is allowed to read.
- Prefer an XPC service boundary if the shipped app needs stricter privilege
  separation.

For remembered access:

- Create a security-scoped bookmark for the selected directory.
- Store bookmark data as sensitive capability metadata.
- Resolve the bookmark on launch.
- Detect stale bookmarks and require re-selection when needed.
- Call `startAccessingSecurityScopedResource()` before host-side access.
- Balance access with `stopAccessingSecurityScopedResource()`.
- Delete bookmark data on revocation.

If Herm remains an unsandboxed CLI/dev tool, keep the same explicit
picker/bookmark registry as product policy. Do not scan broad iCloud Drive paths
just because POSIX permissions allow it.

### Hydration

Before exposing a macOS directory as a live mount or snapshot:

- Recursively enumerate regular files.
- Check whether each item is ubiquitous when resource keys are available.
- Check download status and unresolved-conflict status.
- Request download for files that are not local/current.
- Surface progress, failure, and cancellation.
- Recheck hydration immediately before staged copy or write-back.

Folder-level checks are not enough. A directory may contain a mix of downloaded
files and placeholders.

### File Coordination and Conflicts

The host bridge must use file coordination for host-side copy, snapshot,
delete, move, and write-back operations against iCloud URLs. Use a file
presenter or equivalent invalidation model when keeping an active root open.

For every snapshot or live mount, record a preflight manifest:

- virtual path
- display path
- file type
- size
- modified time
- content hash when practical
- download/materialization state
- unresolved-conflict flag

On write-back:

- Reject stale writes if the manifest no longer matches.
- Detect conflicts before applying changes.
- Prompt the user rather than silently overwriting.
- Use host-side coordinated writes.
- Require explicit approval for delete, rename, overwrite, bulk operations, and
  package-directory changes.

## iOS and iPadOS Bridge Requirements

### Authorization

Treat every document-picker URL outside the app sandbox as security-scoped.
Open access only while the host is actively reading, copying, or writing, and
release access as soon as possible.

For remembered iOS/iPadOS directory mounts:

- Store bookmark data in the host app only.
- Resolve the bookmark on launch.
- Detect stale or failed bookmarks.
- Require re-selection when resolution fails.
- Test the exact bookmark flow on real iPhone and iPad devices.
- Expose an in-app revoke control that deletes bookmark data and invalidates
  active local or remote mount sessions.

Do not pass iOS security-scoped URLs, bookmark data, or sandbox extension state
to CPSL.

### Materialization

Selected Files locations may be backed by iCloud Drive or another File Provider.
The bridge must assume reads may require network materialization and may fail
offline.

For snapshot mode:

1. Start security-scoped access for the selected URL.
2. Coordinate a fast recursive copy into app-controlled storage.
3. Release the security scope.
4. Run the local capsule over the staged directory, or upload the staged
   package to a remote CPSL session.

For live-readonly mode:

1. Keep the app foreground-first.
2. Proxy CPSL reads through host code.
3. Coordinate per file operation.
4. Avoid holding a file coordination accessor open while CPSL runs arbitrary
   work.

For staged-writeback:

1. CPSL writes into app-controlled staging.
2. The host presents a review queue.
3. The host reopens security-scoped access.
4. The host checks freshness/conflicts.
5. The host applies approved changes through coordinated writes.

### Background Limits

Do not rely on iOS background execution to maintain a live directory proxy.

- Local live mounts require the app to be active or recently backgrounded.
- Finite background tasks are for short completion windows, not indefinite
  filesystem service.
- Background task scheduling is system-controlled and not immediate.
- Remote sessions should upload a snapshot or staged package before the agent
  depends on the content.
- Background `URLSession` can continue large transfers, but it is not a
  substitute for a live mounted filesystem.

## Prompt and Agent Context

When any iCloud mount is active, the system prompt should include an explicit
mount table:

```text
Available mounts:
- /workdir: writable workspace
- /icloud/project: read-only iCloud Drive directory, label "Project", state downloaded
- /icloud-output/project: writable staged output for Project
```

Prompt rules:

- Read source files from `/icloud/<name>`.
- Write generated outputs to `/workdir` or `/icloud-output/<name>`.
- Do not modify `/icloud/<name>` directly unless an explicit advanced
  read-write mode is active.
- Do not assume files outside the listed mount table exist.
- Treat iCloud files as personal data.
- Do not use network tools to send iCloud content unless the user explicitly
  approved that destination and purpose.

Subagents must receive the same mount table and network policy.

## Network Policy

iCloud-mounted sessions should default to no outbound network access.

Requirements:

- Empty allow list by default for CPSL HTTP policy.
- Provider-side web search/server tools disabled while iCloud mounts are active
  unless there is a separate explicit exfiltration approval model.
- Any enabled destination must be shown to the user.
- Audit attempted and allowed network destinations.
- Treat copying iCloud content to a remote CPSL session as an explicit upload
  consent event, separate from local mounting.

## Security Model

Treat iCloud Drive as high-risk personal data.

Required controls:

- Explicit user-selected directory scope.
- Read-only default.
- Stable virtual paths instead of raw Apple platform paths.
- No Apple credentials in CPSL.
- No security-scoped URLs or bookmark data in CPSL.
- No broad network egress while iCloud content is mounted.
- Staged write-back with approval.
- Revocation that stops access and removes future mount visibility.
- Audit logs that avoid file contents by default.

Required audit events:

- Scope selected.
- Bookmark stored, refreshed, failed, or revoked.
- Mount started and stopped.
- Files read or copied, summarized as path plus metadata.
- Files staged for write-back.
- User approvals and denials.
- Files applied back to iCloud.
- Network destinations attempted or allowed.
- Remote upload consent and transfer status.

Do not log file contents by default. Use content hashes only when needed for
freshness or conflict detection.

## Sync and Consistency

iCloud Drive and File Provider locations are sync systems, not distributed
filesystems with strong locking.

Design assumptions:

- Files may be placeholders until downloaded or materialized.
- Local files may change outside CPSL while a session is running.
- Other devices may edit the same files.
- Deletes and overwrites can propagate to other devices.
- Conflicts can occur and must be surfaced to the user.
- Package directories, resource forks, extended attributes, Finder tags,
  permissions, and provider metadata may not round-trip through staging or
  non-Apple tools.
- iOS paths and provider URLs are implementation details and must not be shown
  to the agent.

The first implementation should prefer snapshots or read-only live mounts.
Writable live iCloud mounts should wait until there is a single-writer rule,
coordinated write broker, conflict UI, and tested rollback behavior.

## UX Requirements

The UI should show:

- Platform: macOS, iOS, or iPadOS.
- Provider/location: iCloud Drive, On My iPhone/iPad, external storage, or
  third-party provider.
- Selected display path.
- Virtual CPSL path.
- Access mode: snapshot, live-readonly, staged-writeback, or advanced direct
  read-write.
- Session-only or remembered access.
- Download/materialization state.
- File count and size estimate.
- Freshness/conflict state.
- Network policy state.
- Remote upload status when applicable.
- Last access time.
- Revoke control.
- Pending write-back queue.

Before remote use, show a separate consent step:

```text
Upload selected files to this remote CPSL session?
```

That consent is separate from local iCloud directory selection.

## Implementation Plan

Phase 1: platform-neutral mount model

- Define `AuthorizedMount`.
- Add validation for `/icloud/<name>` virtual paths.
- Add prompt rendering for active mounts.
- Add audit event types for iCloud scope and mount lifecycle.

Phase 2: macOS snapshot import

- Add macOS folder selection.
- Support session-only selected directory grants.
- Coordinate and copy selected files into `.herm/icloud/<scope>/`.
- Expose the staged content through `/workdir` or an extra read-only CPSL
  mount.
- Keep writes separate under `.herm/icloud-output/<scope>/`.

Phase 3: Herm extra scoped mounts

- Extend Herm's CPSL config model to carry multiple mount specs.
- Extend the worker CLI beyond `--workspace`, or pass a session config file.
- Build CPSL FFI session JSON with `/workdir` plus `/icloud/<name>` mounts.
- Add read-only iCloud mount tests.
- Update CPSL prompts that currently assume only `/workdir`.

Phase 4: macOS remembered access and staged write-back

- Store and resolve macOS security-scoped bookmarks.
- Add revocation UI.
- Add write-back queue.
- Add manifest/freshness checks.
- Apply approved writes through host-side coordinated operations.

Phase 5: iOS/iPadOS snapshot bridge

- Add directory/file selection through the document picker.
- Coordinate a snapshot into app-controlled storage.
- Run local CPSL on the staged tree where available, or upload the staged tree
  to a remote CPSL session after explicit consent.
- Add real-device tests for bookmark persistence and provider behavior.

Phase 6: iOS/iPadOS staged write-back

- Add review queue.
- Reopen selected directory access.
- Detect stale bookmarks and conflicts.
- Apply approved changes through coordinated writes.
- Add failure recovery for app suspension, provider unavailability, and offline
  state.

Phase 7: optional live-readonly proxy

- Add a brokered file operation API for local on-device CPSL.
- Coordinate per read/list/stat operation.
- Enforce read-only in the bridge, not only in CPSL.
- Disable or degrade when the app is backgrounded.

## Test Plan

Functional tests:

- macOS iCloud Drive folder selection.
- macOS remembered bookmark relaunch.
- iOS iCloud Drive folder selection.
- iPadOS Files directory selection.
- On My iPhone/iPad folder selection.
- Third-party File Provider folder selection.
- External storage or file server location when supported.
- Multiple selected directories.
- Files with spaces and Unicode names.
- Package directories.
- Large files.
- Placeholder-only files.
- Offline mode.
- Revocation during an active session.

Security tests:

- Attempt `..` traversal from mounted paths.
- Attempt symlink escape.
- Attempt writes to read-only mounted iCloud content.
- Attempt bulk delete.
- Attempt package-directory mutation.
- Verify Apple credentials are never present in CPSL environment, files, logs,
  prompts, or trace output.
- Verify security-scoped URLs and bookmark data are never sent to CPSL.
- Verify raw iOS container paths are hidden from prompts and logs.
- Verify remote token revocation stops access.
- Attempt network exfiltration with network disabled.

Sync and conflict tests:

- Modify a file outside CPSL while a session is running.
- Delete a file outside CPSL while staged write-back is pending.
- Edit the same file on another Apple device.
- Create same-name files on two devices.
- Verify stale write-back is refused.
- Verify conflict UI is triggered.

Operational tests:

- Mac goes offline.
- iPhone/iPad goes offline.
- iCloud Drive is disabled.
- Files privacy access is revoked.
- Optimize Mac Storage evicts a file.
- A provider fails to materialize a file.
- App is suspended during remote upload.
- App is terminated and relaunched with remembered access.
- Advanced Data Protection is enabled.

## Open Questions

- Should v1 expose selected iCloud files as attachments, staged snapshots, or
  true extra read-only mounts?
- Should macOS remembered access ship in v1 or start session-only?
- Should iOS/iPadOS remembered access ship before remote upload support?
- Should Herm pass extra mounts through repeated CLI flags or a session config
  JSON file?
- What audit storage should exist for personal iCloud access?
- What enterprise policy controls are needed for personal cloud-drive mounts?
- Should provider-side web/search tools be categorically disabled for any
  session with an iCloud mount?
- Can live-readonly proxying be implemented without weakening CPSL's simple
  mount-table model?

## Decision

Proceed with an Apple-hosted scoped directory mount design.

The first milestone should be macOS snapshot or read-only selected-folder
mounts. iOS/iPadOS should follow with document-picker snapshot support and
explicit remote upload consent. Extra scoped CPSL mounts are the long-term
architecture, but Herm must first grow a multi-mount worker/session interface
and prompt context for active mounts.

Do not pursue a full arbitrary iCloud Drive mount as a server-side or Linux
CPSL feature.

## References

Repo references:

- `README.md`: CPSL is Unix-like enough for agents, with explicit modules,
  files, mounts, and network rules.
- `core/src/mount.rs`: CPSL mount table and read-only/path traversal
  enforcement.
- `ffi/src/lib.rs`: CPSL FFI session config and multi-mount validation.
- `cli/src/config.rs`: manifest `[mounts].volumes` config.
- `cli/src/main.rs`: CLI volume mounting.
- `herm/cmd/herm/cpsl_worker.go`: current Herm CPSL worker options.
- `herm/cmd/herm/cpsl_client.go`: current Herm CPSL worker process args.
- `herm/cmd/herm/cpsl_library.go`: current Herm CPSL session JSON construction.
- `herm/cmd/herm/promptprofile.go`: CPSL prompt profile currently centered on
  `/workdir`.

Apple references:

- [Apple Developer Documentation: `NSOpenPanel`](https://developer.apple.com/documentation/appkit/nsopenpanel).
- [Apple Developer Documentation: Accessing files from the macOS App Sandbox](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox).
- [Apple Developer Documentation: Security-scoped resource access](<https://developer.apple.com/documentation/foundation/url/startaccessingsecurityscopedresource()>).
- [Apple Developer Documentation: `UIDocumentPickerViewController`](https://developer.apple.com/documentation/UIKit/UIDocumentPickerViewController).
- [Apple Developer Documentation: Providing access to directories](https://developer.apple.com/documentation/uikit/providing-access-to-directories).
- [Apple Documentation Archive: Document Picker Programming Guide, Accessing Documents](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/DocumentPickerProgrammingGuide/AccessingDocuments/AccessingDocuments.html).
- [Apple Documentation Archive: File System Programming Guide, iCloud File Management](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/iCloud/iCloud.html).
- [Apple Documentation Archive: Designing for Documents in iCloud](https://developer.apple.com/library/archive/documentation/General/Conceptual/iCloudDesignGuide/Chapters/DesigningForDocumentsIniCloud.html).
- [Apple Technical Note TN2336: Handling version conflicts in the iCloud environment](https://developer.apple.com/library/archive/technotes/tn2336/_index.html).
- [Apple Developer Documentation: `FileManager.startDownloadingUbiquitousItem`](<https://developer.apple.com/documentation/foundation/filemanager/startdownloadingubiquitousitem(at:)>).
- [Apple Developer Documentation: File Provider](https://developer.apple.com/documentation/fileprovider).
- [Apple Developer Documentation: Background Tasks](https://developer.apple.com/documentation/backgroundtasks).
- [Apple Developer Documentation: Background `URLSession`](<https://developer.apple.com/documentation/foundation/urlsessionconfiguration/background(withidentifier:)>).
- [Apple Support: Work with folders and files in iCloud Drive on Mac](https://support.apple.com/guide/mac-help/work-with-folders-and-files-in-icloud-drive-mchl1a02d711/mac).
- [Apple Support: Use iCloud Drive on iPhone](https://support.apple.com/guide/iphone/use-icloud-drive-iphe9aff429a/ios).
