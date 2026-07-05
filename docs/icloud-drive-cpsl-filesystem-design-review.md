# iCloud Drive CPSL Filesystem Design Review

Status: three-reviewer consensus

Source document: `docs/icloud-drive-cpsl-filesystem-design.md`

## Summary

Three independent review passes agreed that the design is directionally sound,
but several contracts need to be tightened before implementation. The highest
risk areas are direct provider-backed live mounts, read-only separation for
staged iCloud data, staged write-back safety, and revocation behavior for
in-flight work and remote uploads.

## Consensus Findings

### 1. Direct `live-readonly` mounts are not safe as ordinary CPSL host-path mounts

Severity: High

The document allows CPSL to receive a read-only live mount of a selected iCloud
directory, but later warns that a child CPSL worker may not inherit post-launch
PowerBox access and says iCloud I/O should stay in the host bridge or use staged
directories.

The iOS/iPadOS section describes per-operation security-scoped access through
the host app, but the neutral mount contract and target CPSL session shape are
host-path based.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:73`
- `docs/icloud-drive-cpsl-filesystem-design.md:100`
- `docs/icloud-drive-cpsl-filesystem-design.md:158`
- `docs/icloud-drive-cpsl-filesystem-design.md:186`
- `docs/icloud-drive-cpsl-filesystem-design.md:245`
- `docs/icloud-drive-cpsl-filesystem-design.md:340`

Recommendation:

Make v1 snapshot/staged by default. Any live-readonly support should be defined
as a host-brokered, file-coordinated filesystem API with explicit ownership of
security-scoped access and Apple platform authorization.

### 2. Phase 2 weakens read-only separation for iCloud-origin staged data

Severity: High

The summary, consensus, and prompt rules say iCloud inputs should live under
read-only `/icloud/<name>` mounts and that writes should go to `/workdir` or a
separate output mount. Phase 2 says staged content may be exposed through
`/workdir` and stored under `.herm/icloud/<scope>/`.

If `.herm` is inside a writable `/workdir`, the agent may be able to modify
iCloud-origin staged inputs through the writable workspace path, bypassing the
intended read-only boundary.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:24`
- `docs/icloud-drive-cpsl-filesystem-design.md:48`
- `docs/icloud-drive-cpsl-filesystem-design.md:381`
- `docs/icloud-drive-cpsl-filesystem-design.md:503`

Recommendation:

Require iCloud-origin inputs to mount read-only under `/icloud/<name>` from
storage outside any writable CPSL mount, or define an explicit exclusion and
enforcement mechanism for staged input directories.

### 3. Staged write-back is underspecified

Severity: High

The manifest and write-back rules do not define a concrete operation schema,
strong freshness validators, transaction or commit logging, atomic replacement
behavior, partial failure recovery, package-directory semantics, create/rename/
delete ordering, or whether write-back success means local coordinated apply or
confirmed provider/iCloud sync completion.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:285`
- `docs/icloud-drive-cpsl-filesystem-design.md:296`
- `docs/icloud-drive-cpsl-filesystem-design.md:516`

Recommendation:

Add explicit `SnapshotManifest` and `WritebackPlan` models. Destructive
write-back operations should require strong per-file validators, such as content
hashes of bytes read and current destination bytes, file identity or version
metadata when available, unresolved conflict checks, and directory-entry
fingerprints for creates, deletes, and renames.

Write-back should also define transaction logging, operation ordering, atomic
temp-file replacement, crash recovery, package handling, and what remains
pending after partial failure.

### 4. Revocation lacks an in-flight/session state machine

Severity: High/Medium

The revoke sequence stops new turns, closes the worker, removes descriptors,
deletes staged copies when appropriate, stops security-scoped access, and
deletes remembered bookmarks. It does not define behavior for active evals,
subagents, coordinated reads, open file descriptors, uploads, pending write-back
queues, cancellation failures, or remote sessions that already received bytes.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:220`
- `docs/icloud-drive-cpsl-filesystem-design.md:226`
- `docs/icloud-drive-cpsl-filesystem-design.md:257`
- `docs/icloud-drive-cpsl-filesystem-design.md:309`
- `docs/icloud-drive-cpsl-filesystem-design.md:405`

Recommendation:

Add a mount/session lifecycle state machine with states such as `selected`,
`hydrating`, `active`, `revoking`, `revoked`, and `failed`. Define allowed
transitions, cleanup guarantees, audit events, deadlines for active operations,
and explicit semantics for remote uploaded snapshots.

### 5. Bridge-side link, package, and host-overlap policy is not concrete enough

Severity: Medium

The document says to recursively enumerate regular files and test symlink
escape, and it acknowledges package directories, resource forks, extended
attributes, Finder tags, permissions, and provider metadata may not round-trip.
It does not define how the bridge handles symlinks, Finder aliases, hardlinks,
package contents, canonical host path overlap, duplicate host roots, or writable
parent mounts containing read-only staged roots.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:267`
- `docs/icloud-drive-cpsl-filesystem-design.md:451`
- `docs/icloud-drive-cpsl-filesystem-design.md:570`

Recommendation:

Specify fail-closed bridge behavior. By default, do not follow links outside the
selected root. Either reject symlinks, aliases, and hardlinks, or copy only
in-scope targets after non-following metadata checks and explicit user approval.
Reject unsafe canonical host-path overlap between writable and read-only mounts.

### 6. Provider scope and namespace are inconsistent

Severity: Medium

The iOS/iPadOS selection flow can return iCloud Drive, On My iPhone/iPad,
external storage, file servers, or third-party File Provider locations. The
mount namespace and example source kind remain `/icloud/<name>` and
`icloud-drive-directory`.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:89`
- `docs/icloud-drive-cpsl-filesystem-design.md:158`
- `docs/icloud-drive-cpsl-filesystem-design.md:174`
- `docs/icloud-drive-cpsl-filesystem-design.md:328`

Recommendation:

Decide whether v1 is iCloud-only or a broader Apple Files/File Provider bridge.
If it is general, add provider/source enums and consider a neutral namespace
such as `/files/<name>`.

### 7. Network and server-tool policy is both a requirement and an open question

Severity: Medium

The requirements say provider-side web search and server tools are disabled
while iCloud mounts are active unless a separate exfiltration approval model
exists. The open questions later ask whether those tools should be
categorically disabled for any session with an iCloud mount.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:389`
- `docs/icloud-drive-cpsl-filesystem-design.md:401`
- `docs/icloud-drive-cpsl-filesystem-design.md:612`

Recommendation:

Make this a v1 decision rather than both a requirement and an open question.
Define one approval boundary for CPSL HTTP, provider-side tools, LLM context,
and remote uploads.

### 8. `/icloud-output/<name>` lacks a validation and lifecycle contract

Severity: Medium/Low

The sample session and prompt rules use writable output mounts, but the
validation rules only specify `/icloud/<name>` input mounts.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:172`
- `docs/icloud-drive-cpsl-filesystem-design.md:193`
- `docs/icloud-drive-cpsl-filesystem-design.md:378`
- `docs/icloud-drive-cpsl-filesystem-design.md:384`

Recommendation:

Add output-mount validation and lifecycle rules: slug matching, duplicate and
shadow rejection, app-controlled host storage, cleanup behavior, review
semantics, and linkage to write-back plans.

### 9. Audit and observability need a privacy-preserving schema

Severity: Medium/Low

The document lists required audit events but does not define event IDs,
redaction levels, path hashing or redaction, byte counts, correlation IDs,
retention, encryption, access control, user export/delete behavior, or error
codes. `Path plus metadata` can still expose sensitive personal information.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:422`
- `docs/icloud-drive-cpsl-filesystem-design.md:424`
- `docs/icloud-drive-cpsl-filesystem-design.md:429`
- `docs/icloud-drive-cpsl-filesystem-design.md:436`

Recommendation:

Add a structured audit schema with privacy defaults. Avoid file contents by
default, minimize path disclosure, and define retention, export, deletion, and
access-control behavior.

### 10. Implementation and test planning need compatibility and fault-injection structure

Severity: Medium/Low

Phase 3 has no migration or versioning path for existing single-workspace
workers, stored bookmarks, persisted sessions, prompt snapshots, traces, or
config schemas. The test plan lists useful scenarios, but it does not separate
unit, contract, integration, real-device, simulator, and manual tests. It also
does not define acceptance criteria for disk-full partial copies, provider
failure, stale bookmarks, worker restart during revoke, app suspension during
snapshot, or partial write-back recovery.

The security test for remote token revocation is not backed by a token or
remote-session design in the document.

References:

- `docs/icloud-drive-cpsl-filesystem-design.md:508`
- `docs/icloud-drive-cpsl-filesystem-design.md:548`
- `docs/icloud-drive-cpsl-filesystem-design.md:578`

Recommendation:

Add schema/versioning and compatibility requirements for the Herm worker and
prompt/session model. Split tests into CI-safe contract tests, mocked provider
fault tests, macOS integration tests, and real-device iOS/iPadOS tests. Replace
the remote token revocation test or define the remote token/session model first.
