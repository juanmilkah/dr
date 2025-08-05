// dr -l          list all dropped files
// dr -r foo.txt  recover the file
// dr foo.txt     drop the file
// dr -d foo.txt  delete forever

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const ROOT_DIR: &str = "/tmp/dr";
const USAGE: &str = r#"
    dr - Drop files from current path until next reboot after which they are
         permanently deleted from the file system.
    All commands are exclusive.
        default       Drop filepaths from current fs.
        --delete, -d  Delete a filepath permanently.
        --recover, -r Recover a previously dropped fs entry.
        --list, -l    List all droppped filepaths.
    Examples:
        dr foo.txt     drop the file
        dr -r foo.txt  recover the file
        dr -d foo.txt  delete forever
        dr -l          list all dropped files
"#;

#[derive(Debug)]
struct Cli {
    command: Command,
    filepaths: Option<Vec<PathBuf>>,
}

#[derive(Debug, PartialEq)]
enum Command {
    // default
    Drop,
    Delete,
    Recover,
    List,
    Help,
}

trait Parse {
    type Item;

    fn parse(args: impl Iterator<Item = String>) -> Result<Self::Item, String>;
}

impl Parse for Cli {
    type Item = Cli;

    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self::Item, String> {
        // defaults
        let mut command = Command::Drop;
        let mut filepaths = Vec::new();

        if let Some(nxt) = args.next() {
            match nxt.as_str() {
                "--list" | "-l" => command = Command::List,
                "--recover" | "-r" => {
                    command = Command::Recover;
                    filepaths.extend(args.map(|a| Path::new(&a).to_path_buf()));
                }

                "--delete" | "-d" => {
                    command = Command::Delete;
                    filepaths.extend(args.map(|a| Path::new(&a).to_path_buf()));
                }

                "--help" | "-h" => command = Command::Help,

                other => {
                    command = Command::Drop;

                    filepaths.push(Path::new(&other).to_path_buf());
                    filepaths.extend(args.map(|a| Path::new(&a).to_path_buf()));
                }
            }
        }

        if command != Command::List && filepaths.is_empty() {
            return Err("Missing filepaths".to_string());
        }

        Ok(Cli {
            command,
            filepaths: if filepaths.is_empty() {
                None
            } else {
                Some(filepaths)
            },
        })
    }
}

fn main() {
    let args = match Cli::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    if !Path::new(ROOT_DIR).exists() {
        fs::create_dir(ROOT_DIR).expect("Failed to create dr default dir");
    }

    match args.command {
        Command::Drop => drop_entries(&args.filepaths.unwrap()),
        Command::Delete => delete_entries(&args.filepaths.unwrap()),
        Command::Recover => recover_entires(&args.filepaths.unwrap()),
        Command::List => list_entries(),
        Command::Help => println!("{USAGE}"),
    }
}

fn list_entries() {
    let entries = match fs::read_dir(ROOT_DIR) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    for e in entries {
        let e = match e {
            Ok(e) => e,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };
        let e = e.path();
        println!("{e}", e = e.display());
    }
}

fn recover_entires(filepaths: &[PathBuf]) {
    let dropped_files = match fs::read_dir(ROOT_DIR) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    let dropped_paths: Vec<(PathBuf, PathBuf)> = dropped_files
        .flatten()
        .map(|p| p.path().to_string_lossy().to_string())
        .filter_map(|s| {
            let old_name = Path::new(s.as_str().split_once("_").unwrap().1).to_path_buf();
            if filepaths.contains(&old_name) {
                let s = Path::new(&s).to_path_buf();
                Some((s, old_name))
            } else {
                None
            }
        })
        .collect();

    for (current, old_name) in dropped_paths {
        if !old_name.exists() {
            eprintln!(
                "File: {old_name} already exists!",
                old_name = old_name.display()
            );
            continue;
        }

        if let Err(e) = fs::copy(current, &old_name) {
            eprintln!("{e}");
        }
        println!("Recovered: {old_name}", old_name = &old_name.display());
    }
}

fn delete_entries(filepaths: &[PathBuf]) {
    // Dropped entries are saved as `{date}_{old_filename}` in the root_dir
    let dropped_files = match fs::read_dir(ROOT_DIR) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    let dropped_paths: Vec<PathBuf> = dropped_files
        .flatten()
        .map(|p| p.path().to_string_lossy().to_string())
        .filter_map(|s| {
            // This unwrap could be triggered if something tempered with the
            // naming convection in the root_dir
            let old_name = Path::new(s.as_str().split_once("_").unwrap().1).to_path_buf();
            if filepaths.contains(&old_name) {
                let current = Path::new(&s).to_path_buf();
                Some(current)
            } else {
                None
            }
        })
        .collect();

    for p in dropped_paths {
        if p.is_dir() {
            if let Err(e) = fs::remove_dir_all(p) {
                eprintln!("{e}");
            }
        } else if let Err(e) = fs::remove_file(p) {
            eprintln!("{e}");
        }
    }
}

// Dropped entries are saved as `{date}_{old_filename}` in the root_dir
fn drop_entries(filepaths: &[PathBuf]) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    for f in filepaths {
        let name = format!("{now}_{f}", f = f.display());
        let name = Path::new(ROOT_DIR).join(name);

        if let Err(e) = fs::rename(f, &name) {
            if e.kind() == io::ErrorKind::CrossesDevices {
                //try copy
                if let Err(e) = fs::copy(f, &name) {
                    eprintln!("Failed kernel based file copy: {e}");
                    break;
                }
            }
            eprintln!("Failed rename: {e}");
        }

        if f.is_dir() {
            let _ = fs::remove_dir_all(f);
        } else {
            let _ = fs::remove_file(f);
        }
    }
}
