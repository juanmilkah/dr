// dr -l          list all dropped files
// dr -r foo.txt  recover the file
// dr foo.txt     drop the file
// dr -d foo.txt  delete forever

use std::{
    env, fs, io,
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
        let mut command = Command::Drop;
        let mut filepaths = Vec::new();

        if let Some(nxt) = args.next() {
            match nxt.as_str() {
                "--list" | "-l" => command = Command::List,
                "--recover" | "-r" => {
                    command = Command::Recover;
                    filepaths.extend(args.map(|a| Path::new(&a).to_path_buf()).map(|p| {
                        if p.is_absolute() {
                            p
                        } else {
                            env::current_dir().unwrap().join(p)
                        }
                    }));
                }

                "--delete" | "-d" => {
                    command = Command::Delete;
                    filepaths.extend(args.map(|a| Path::new(&a).to_path_buf()).map(|p| {
                        if p.is_absolute() {
                            p
                        } else {
                            env::current_dir().unwrap().join(p)
                        }
                    }));
                }

                "--help" | "-h" => command = Command::Help,

                other => {
                    command = Command::Drop;

                    let path = Path::new(other);
                    filepaths.push(if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        env::current_dir().unwrap().join(path)
                    });

                    filepaths.extend(args.map(|a| Path::new(&a).to_path_buf()).map(|p| {
                        if p.is_absolute() {
                            p
                        } else {
                            env::current_dir().unwrap().join(p)
                        }
                    }));
                }
            }
        }

        if matches!(command, Command::Drop | Command::Delete | Command::Recover)
            && filepaths.is_empty()
        {
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

    let root = Path::new(ROOT_DIR);
    if !root.exists() {
        fs::create_dir(ROOT_DIR).expect("Failed to create dr default dir");
    }

    match args.command {
        Command::Drop => drop_entries(&args.filepaths.unwrap(), root),
        Command::Delete => delete_entries(&args.filepaths.unwrap(), root),
        Command::Recover => recover_entries(&args.filepaths.unwrap(), root),
        Command::List => list_entries(root),
        Command::Help => println!("{USAGE}"),
    }
}

fn list_entries(root: &Path) {
    let entries = match fs::read_dir(root) {
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

        let path = e.path();
        let filename = path.file_name().unwrap().to_string_lossy();

        if let Some((_, original_name)) = filename.split_once('_') {
            println!("{original_name}");
        } else {
            println!("{filename}");
        }
    }
}

fn recover_entries(filepaths: &[PathBuf], root: &Path) {
    let dropped_files = match fs::read_dir(root) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    let dropped_paths: Vec<(PathBuf, PathBuf)> = dropped_files
        .flatten()
        .filter_map(|entry| {
            let stored_path = entry.path();
            let filename = stored_path.file_name()?.to_string_lossy();

            let (_, original_name) = filename.split_once('_')?;
            let original_path = PathBuf::from(original_name);

            if filepaths.contains(&original_path) {
                Some((stored_path, original_path))
            } else {
                None
            }
        })
        .collect();

    for (stored_path, original_path) in dropped_paths {
        if original_path.exists() {
            eprintln!("File already exists: {}", original_path.display());
            continue;
        }

        if let Some(parent) = original_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Failed to create directories: {e}");
                continue;
            }
        }

        if let Err(e) = fs::rename(&stored_path, &original_path) {
            eprintln!("Failed to recover {}: {e}", original_path.display());
        } else {
            println!("Recovered: {}", original_path.display());
        }
    }
}

fn delete_entries(filepaths: &[PathBuf], root: &Path) {
    let dropped_files = match fs::read_dir(root) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error reading dropped files: {e}");
            return;
        }
    };

    let dropped_paths: Vec<PathBuf> = dropped_files
        .flatten()
        .filter_map(|entry| {
            let stored_path = entry.path();
            let filename = stored_path.file_name()?.to_string_lossy();

            let (_, original_name) = filename.split_once('_')?;
            let original_path = PathBuf::from(original_name);

            if filepaths.contains(&original_path) {
                Some(stored_path)
            } else {
                None
            }
        })
        .collect();

    for path in dropped_paths {
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };

        if let Err(e) = result {
            eprintln!("Failed to delete {}: {e}", path.display());
        } else {
            println!("Permanently deleted: {}", path.display());
        }
    }
}

fn drop_entries(filepaths: &[PathBuf], root: &Path) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    for filepath in filepaths {
        if !filepath.exists() {
            eprintln!("File not found: {}", filepath.display());
            continue;
        }

        let abs_path = filepath
            .canonicalize()
            .unwrap_or_else(|_| filepath.to_path_buf());
        let stored_name = format!("{now}_{}", abs_path.to_string_lossy());
        let stored_path = root.join(stored_name);

        if let Err(e) = fs::rename(filepath, &stored_path) {
            if e.kind() == io::ErrorKind::CrossesDevices {
                let copy_result = if filepath.is_dir() {
                    copy_dir_all(filepath, &stored_path)
                } else {
                    fs::copy(filepath, &stored_path).map(|_| ())
                };

                if let Err(e) = copy_result {
                    eprintln!("Failed to copy {}: {e}", filepath.display());
                    continue;
                }

                let remove_result = if filepath.is_dir() {
                    fs::remove_dir_all(filepath)
                } else {
                    fs::remove_file(filepath)
                };

                if let Err(e) = remove_result {
                    eprintln!("Failed to remove original {}: {e}", filepath.display());
                    let _ = fs::remove_file(&stored_path);
                    continue;
                }
            } else {
                eprintln!("Failed to drop {}: {e}", filepath.display());
                continue;
            }
        }

        println!("Dropped: {}", filepath.display());
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
