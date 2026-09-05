# Subagent visibility monitor

This fork includes a native Codex TUI monitor for parallel subagents. It is
opened with `/subagents` and shows each child agent's explicit status, with an
optional toggle for completed agents. The monitor has no tmux dependency: it is
part of the Codex interface and uses the existing session state.

The view can show cumulative server token usage when the server reports it. If
that information is unavailable, the token field is shown as unknown rather
than inferred. The displayed tool class is the safe current class reported by
the session. Enter opens navigation through the existing transcript. Input
remains owned by the parent session, so selecting a child does not let the
parent accidentally type into or mutate that child's input. Completed agents
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

## Test

From `codex-rs`, run the TUI project's standard test command:

```sh
just test -p codex-tui
```

The command uses the repository's configured test runner and is the relevant
check for the monitor UI.

## Contributing and distributing

Keep this guide at the repository root so it travels with the fork. When
updating the patch, rebase or merge the upstream changes first, resolve any
conflict in the monitor and its documentation together, then rerun the build
and TUI test command. Review the resulting diff before sharing a binary or
opening a pull request. Distribution should identify the exact fork revision
and make clear that the monitor is custom until an upstream implementation is
accepted.
