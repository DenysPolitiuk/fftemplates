extern crate clap;
extern crate dirs;
extern crate fs_extra;
extern crate tempfile;

use clap::App;
use clap::Arg;
use fs_extra::dir;
use fs_extra::dir::CopyOptions;
use tempfile::TempDir;

use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time;
use std::time::Duration;
use std::time::SystemTime;

const HASH_NAME_SPLIT_CHAR: char = '.';

const IGNORE_FILES: [&str; 9] = [
    "cache2",
    "cookies.sqlite-wal",
    "favicons.sqlite-wal",
    "lock",
    "places.sqlite-wal",
    "safebrowsing",
    "sessionstore-backups",
    "startupCache",
    "webappsstore.sqllite-wal",
];

// TODO:
//
// * Have to restart firefox process to get extension. Same behaviour when running
// `firefox --profile <path>` directly
fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("base_profile")
                .short("p")
                .long("profile")
                .help("profile to run")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("delay")
                .short("d")
                .long("delay")
                .help("delay between restarting process (needed for extensions)")
                .takes_value(true),
        )
        .get_matches();

    let profile_name = matches
        .value_of("base_profile")
        .or(Some("default"))
        .unwrap();
    let delay = matches
        .value_of("delay")
        .or(Some("3"))
        .unwrap()
        .parse::<u32>()
        .expect("delay must be a valid number");

    if delay > 60 {
        // unreasonably long delay
        panic!("Delay too long, should be 60 sec or less");
    }

    let profile_folder = Path::new(&dirs::home_dir().unwrap())
        .join(Path::new(".mozilla"))
        .join(Path::new("firefox"));

    if let Err(e) = run(profile_folder, profile_name, delay) {
        println!("Error from run : {}", e);
    }
}

fn run<P: AsRef<Path>>(
    profile_folder: P,
    profile_name: &str,
    process_restart_delay: u32,
) -> Result<(), Box<dyn Error>> {
    let mut ignore_entries = HashSet::new();
    for str_to_ignore in IGNORE_FILES.iter() {
        ignore_entries.insert(*str_to_ignore);
    }

    let tmp_dir = TempDir::new()?;

    let found_profile_pair = find_profile_folder(profile_folder, profile_name)?;

    let (found_profile_path, _) = match found_profile_pair {
        None => Err(format!("No profile with name `{}` found", profile_name))?,
        Some((p, name)) => (p, name),
    };

    let options = CopyOptions::new();
    let start = SystemTime::now();
    // some unique name for new temp profile
    let new_tmp_dir_name = format!("{}", start.duration_since(time::UNIX_EPOCH)?.as_millis());
    let new_tmp_path = tmp_dir.path().join(new_tmp_dir_name);
    dir::create_all(&new_tmp_path, false)?;
    let vec = fs::read_dir(&found_profile_path)?
        .map(|x| x.expect("unable to read profile folder").path())
        .filter_map(|e| {
            let mut valid = false;
            let name = e.as_path().file_name();
            if let Some(name) = name {
                let name = name.to_str();
                if let Some(name) = name {
                    valid = !ignore_entries.contains(name);
                }
            }
            if valid {
                Some(e)
            } else {
                None
            }
        })
        .collect();
    fs_extra::copy_items(&vec, &new_tmp_path, &options)?;

    let command = format!("firefox --profile {}", new_tmp_path.display());

    // first process will have extensions not working properly
    // need to restart the process to have them working
    execute_cmd(&command, true, process_restart_delay)?;
    execute_cmd(&command, false, process_restart_delay)?;

    tmp_dir.close()?;

    Ok(())
}

fn find_profile_folder<P: AsRef<Path>>(
    profile_folder: P,
    profile_name: &str,
) -> Result<Option<(PathBuf, String)>, Box<dyn Error>> {
    let mut found = None;

    for entry in fs::read_dir(profile_folder)? {
        let entry = entry?;
        let entry_path = entry.path();
        let entry_name = entry
            .file_name()
            .into_string()
            .expect("Error during path to string");
        if !entry_name.contains(HASH_NAME_SPLIT_CHAR) {
            continue;
        }
        let name_split: Vec<_> = entry_name.splitn(2, HASH_NAME_SPLIT_CHAR).collect();
        if name_split.len() != 2 {
            panic!(format!(
                "Not split character `{}` in file name",
                HASH_NAME_SPLIT_CHAR
            ));
        }
        let entry_profile_name = name_split[1];
        if entry_profile_name == profile_name {
            found = Some((entry_path, entry_name));
            break;
        }
    }

    Ok(found)
}

pub fn execute_cmd(
    cmd: &String,
    first_time: bool,
    first_time_delay: u32,
) -> Result<(), Box<dyn Error>> {
    let cmd_split: Vec<_> = cmd.split(' ').collect();
    if cmd_split.len() < 1 || cmd_split[0] == "" {
        return Err("No command specified")?;
    }

    let mut proc = Command::new(cmd_split[0])
        .args(&cmd_split[1..cmd_split.len()])
        .spawn()?;

    if cmd_split.len() < 2 {
        proc = Command::new(cmd_split[0]).spawn()?;
    } else {
    }

    if first_time {
        thread::sleep(Duration::from_secs(first_time_delay.into()));
        proc.kill()?;
    } else {
        let _ = proc.wait_with_output()?;
    }

    Ok(())
}
