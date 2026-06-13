# Clabby: command reference

These transcripts are checked against the real `--help` output, so this reference
cannot drift from the binary (Constitution §9). Regenerate after an intentional CLI
change with `TRYCMD=overwrite cargo test -p clabby --test cli_docs`.

## Top level

```console
$ clabby --help
Local command center for agent work, synced to your issue tracker

Usage: clabby[EXE] [OPTIONS] <COMMAND>

Commands:
  sync      Pull issues from the tracker and reconcile them locally
  status    Show the overview dashboard
  issue     Issue operations
  session   Session operations
  worktree  Worktree operations
  logs      Session log inspection
  cron      Cron scheduler for configured [[cron]] jobs
  help      Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>  Path to clabby.toml (default: discovered from the current directory upward)
  -h, --help             Print help
  -V, --version          Print version

```

```console
$ clabby --version
clabby [..]

```

## Sessions

```console
$ clabby session --help
Session operations

Usage: clabby[EXE] session [OPTIONS] <COMMAND>

Commands:
  spawn   Spawn a managed agent run for an issue and stream its output
  attach  Register an externally-run interactive session for the overview
  list    List all known sessions
  help    Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>  Path to clabby.toml (default: discovered from the current directory upward)
  -h, --help             Print help

```

## Cron

```console
$ clabby cron --help
Cron scheduler for configured [[cron]] jobs

Usage: clabby[EXE] cron [OPTIONS] <COMMAND>

Commands:
  run   Run configured jobs. By default starts a scheduler until interrupted; `--once` runs every configured action a single time and exits
  help  Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>  Path to clabby.toml (default: discovered from the current directory upward)
  -h, --help             Print help

```
