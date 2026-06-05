# dev-link

Cross-platform CLI that externalizes a repo's gitignored docs/plans into a
central private repo and replaces them with links. Port of the original
`link-project.ps1` / `relink.ps1`. Runs from any terminal on Windows, Linux, macOS.

## Install

    cargo install --path .

## Configure (once)

    dev-link init --central /path/to/dev-docs

Writes `~/.config/dev-link/config.toml`:

    central = "/path/to/dev-docs"
    items   = [".planning", "docs", ".docs", ".omc"]

Items that don't exist in the project are skipped (a `skip ...` note is printed to stderr).

## Onboard a repo

    cd /path/to/myRepo
    dev-link link              # uses cwd + configured items
    # or explicitly:
    dev-link link --project /path/to/myRepo --items .planning,docs

Moves each item into `<central>/<repo>[/<subpath>]/` and links it back. Nested
paths are mirrored: from `myRepo/src/frontend`, `dev-link link --items .env`
creates `<central>/myRepo/src/frontend/.env`.

## Fresh machine

    git clone <central-remote> /path/to/dev-docs
    cd /path/to/myRepo
    dev-link relink

(Pass `--central /path/to/dev-docs` if you haven't run `dev-link init` on this machine yet.)

## Linking rules

| Item | Linux/macOS | Windows |
|------|-------------|---------|
| directory | symlink | junction if same volume as central (no Developer Mode); else `symlink_dir` (needs Developer Mode ON or admin) |
| file | symlink | `symlink_file` (needs Developer Mode ON or admin) |

The no-Developer-Mode path applies to directory items on the **same volume** as
the central repo (the common case). Cross-volume directories and all file items
fall back to real symlinks, which on Windows need Developer Mode or an admin shell.

`--central` overrides config; if neither is set, `dev-link` errors and asks you
to configure. Requires `git` on PATH.
