---
status: accepted
---

# Verify Linux releases through one coordinated pipeline

Independent CI, GitHub Release, and AUR workflows currently require manual
coordination and can report success before a usable package reaches users. The
agreed direction is a release PR whose approved merge initiates automated checks,
builds, publication, and verification for Linux x86_64 GitHub artifacts and both
AUR packages, `pomotui` and `pomotui-git`. This phase prioritizes that Linux
contract while retaining existing macOS/Homebrew delivery; extending equivalent
end-to-end guarantees to those channels is deferred.

Real installation and upgrade verification must cover clean installation and
upgrading the previous stable package, including Running and Paused Sessions,
Pending Reviews, Focus Reservations, failed services, and deliberately disabled
services. It must establish that configured Timer Services recover without a
manual `systemctl` command, Current Session timing and durable data survive, and
disabled services remain disabled. Existing released/frozen-format migration
obligations in ADR 0011, including the v0.1.0 and v0.2.0 fixture coverage, continue
to apply. Mocked hook tests alone do not establish this upgrade guarantee.

Automatic recovery is guaranteed for supported AUR installations. Preserve user
configuration and data when a source-install unit overrides the packaged unit,
data is damaged, or another unsupported installation conflict prevents recovery;
diagnose the exact conflict and provide actionable remediation instead of
silently deleting or resetting user state.

The project version in `Cargo.toml` is authoritative for stable releases. The
release PR determines that version and its release notes; automation derives and
checks tags, package versions, archive checksums, and `.SRCINFO`. Git-package
versions remain commit-derived snapshots, automatically refreshed when source or
packaging changes on `main` pass verification. Stable publication remains tied
to the approved release PR.

Existing macOS builds continue, but their failures do not gate Linux/AUR release
completion. Linux publication reports success only after its required channels
are verified. When publication partially succeeds, preserve successful outputs,
retry transient failures a bounded number of times, and allow the same version
to resume without moving its tag or changing already published content.

Track channel publication as three distinct stages: pushed, verified against
the authoritative remote content, and discoverable by users. For AUR, verify the
remote recipe and metadata, then wait a bounded time for the package page or
index to expose the version. An indexing timeout remains "pushed, awaiting
indexing" and can resume verification; it is not reported as complete and does
not cause a different version to be published.

Use GitHub-hosted ephemeral runners and a disposable Arch environment for the
real installation/upgrade gate; do not introduce a persistent self-hosted server
or an additional paid service. Prove the environment's feasibility and runtime
before relying on it. Test failure or timeout blocks publication; infrastructure
limitations do not justify silently skipping the gate. If this cannot be made
reliable within the agreed infrastructure boundary, revisit the design before
introducing another service or weakening verification.

Report stable-release channel status on the release PR and Git-package status in
GitHub Actions summaries. Retain diagnostic logs and a same-version retry entry
point so maintainers can identify the failed stage without reconstructing the
whole workflow. Use GitHub's existing failure notifications rather than adding
another notification service.

Before publishing, check AUR credential usability and permission for the target
package. Credential failure pauses publication with actionable diagnostics.
Secret replacement, account permission changes, and SSH host-key changes remain
explicit maintainer actions; routine releases do not require credential setup.

All policy decisions above were confirmed during the design interview. This ADR
records the accepted target; it does not assert that the current workflows meet
it. Implementing the coordinated pipeline, its feasibility experiment, and the
upgrade harness begins after final confirmation of the complete design.
