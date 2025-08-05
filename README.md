# `dr` — Temporary File Drop Utility

`dr` is a simple CLI tool that **temporarily drops files** into a secure location (`/tmp/dr`) until they are either **recovered** or **deleted permanently**. Files are automatically **deleted after a reboot** unless recovered before that.

This is especially useful for staging deletions, quick undos, or temporary file removal in scripts or workflows.

---

## Features

* Drop files without immediately deleting them
* Recover dropped files before they’re lost forever
* Delete dropped files permanently
* List all currently dropped files

---

## Installation

Clone and build:

```bash
git clone https://github.com/yourname/dr.git
cd dr
cargo build --release
cp target/release/dr /usr/local/bin/
```

---

## Usage

```bash
dr [OPTIONS] [FILES...]
```

### Options

| Option            | Description                        |
| ----------------- | ---------------------------------- |
| `-l`, `--list`    | List all currently dropped files   |
| `-r`, `--recover` | Recover dropped file(s)            |
| `-d`, `--delete`  | Permanently delete dropped file(s) |
| (default)         | Drop the file(s) into `/tmp/dr`    |

> All options are exclusive. Use only one at a time.

---

## Examples

```bash
dr foo.txt             # Drop foo.txt into /tmp/dr
dr -l                  # List all dropped files
dr -r foo.txt          # Recover foo.txt to its original location
dr -d foo.txt          # Permanently delete foo.txt from /tmp/dr
```

---

## How It Works

* **Dropped files** are moved to `/tmp/dr` with a timestamp-based prefix.
* **Original file paths** are stored in the filename itself (`<timestamp>_<original path>`).
* On recovery, files are moved back to their original paths (if those paths do not already exist).
* On deletion, files are removed from `/tmp/dr`.
* If no action is taken, the OS will usually clean `/tmp` after a reboot, effectively deleting the files.

---

## Warnings

* **Dropped files are not encrypted** or hidden — they’re just moved to `/tmp/dr`.
* Dropping across file systems uses `copy` + `remove`, not `rename`.
* Ensure `/tmp/dr` exists and has the appropriate permissions.

---

## Cleanup

To remove all dropped files manually:

```bash
rm -rf /tmp/dr
```

---

## License

MIT License. See [MIT](LICENSE) for details.
