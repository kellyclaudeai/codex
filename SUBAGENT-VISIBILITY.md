# Subagent visibility monitor

This fork includes a native Codex TUI monitor for parallel subagents. It is
opened with `/subagents` and shows each child agent's explicit status, with an
optional toggle for completed agents. The monitor has no tmux dependency: it is
part of the Codex interface and uses the existing session state.

The view can show cumulative server token usage when the server reports it. If
that information is unavailable, the token field is shown as unknown rather
than inferred. The displayed tool class is the safe current class reported by
the session. Enter opens navigation through the existing transcript. Existing input permissions remain in force: parent-owned children are view-only;
agents that accept direct input retain their existing follow-up behavior. Completed agents
are hidden or shown with the completed toggle.

This is currently a custom native Codex fork patch. It is not an official
Codex plugin, and this guide makes no claim that upstream Codex supports the
feature or that prebuilt binaries exist.

## Build and run

Use the repository's current Rust toolchain. The Rust workspace also expects
`just` and `cargo-nextest` for its documented workflows. From `codex-rs`, build
the CLI with:

```sh
cargo build -p codex-cli
```

Run the resulting development binary with:

```sh
target/debug/codex
```

If you install it alongside another Codex checkout, use a distinct executable
name such as `codex-visibility` so the fork is easy to identify. Keep the
installation choice local to your environment; this document does not assume
a particular binary directory.

## Companion runtime

Code Mode uses the separate `codex-code-mode-host` executable. The source-build
command is `cargo build -p codex-code-mode-host`, but this base revision's
macOS V8 archive currently returns HTTP 404. That optional helper build is a
separate upstream dependency issue; the native CLI builds successfully.

The verified local installation copies the official Codex 0.153.4 package
layout into an isolated directory and replaces only `bin/codex` with this
fork's executable. Its official companion helper passed the current version-1
stdio handshake, session open, JavaScript expression execution, and clean
shutdown. Tool delegation, cancellation, and limits were not separately tested.
Keep the package manifest and companion resources together when using this
approach, and label the installation as a custom build. Source builds display
`codex-cli 0.0.0`; identify the exact source commit separately.

## Test

From `codex-rs`, run the TUI project's standard test command:

```sh
just test -p codex-tui
```

The command uses the repository's configured test runner and is the relevant
check for the monitor UI. Targeted checks can use:

```sh
just test -p codex-tui -E 'test(agent_monitor) | test(agent_picker)'
```

New children spawned during the connection stream updates without needing to
select them first. Historical descendants discovered on reopening a session may
have only status metadata until their transcript is attached. Unavailable usage
is never inferred from transcript size.

## Contributing and distributing

Keep this guide at the repository root so it travels with the fork. When
updating the patch, rebase or merge the upstream changes first, resolve any
conflict in the monitor and its documentation together, then rerun the build
and TUI test command. Review the resulting diff before sharing a binary or
opening a pull request. Distribution should identify the exact fork revision
and make clear that the monitor is custom until an upstream implementation is
accepted.

## Validation

On macOS arm64, the modified TUI suite ran 4,295 tests: 4,284 passed and 11
failed, with 6 skipped. All seven added monitor tests passed. The unchanged
upstream base (`e01f38c`) reproduced exactly the same 11 keyboard/cursor and
related UI failures. Those baseline snapshots were not changed by this patch.
The new coverage includes event-handler-driven updates while the picker is
open, cumulative token replacement, stale completion handling, safe tool
labels, completion folding, transcript selection, and narrow rendering.

`just fix -p codex-tui` and `just fmt` passed. The compiled CLI was also launched
interactively and its `/subagents` panel opened successfully. No model request
was needed for these smoke checks.
