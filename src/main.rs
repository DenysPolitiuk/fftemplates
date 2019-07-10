extern crate clap;
extern crate dirs;
extern crate fs_extra;
extern crate tempfile;

use clap::App;
use clap::Arg;
use fs_extra::dir;
use fs_extra::dir::CopyOptions;
use tempfile::TempDir;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

const HASH_NAME_SPLIT_CHAR: char = '.';

// TODO:
//
// * No extensions. Have to restart firefox process to get extension. Same behaviour when running
// `firefox --profile <path>` directly
fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("base_profile")
                .short("p")
                .long("profile")
                .help("profile to run")
                .takes_value(true)
                .required(true),
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
        .expect("No base profile provided");
    let delay = matches
        .value_of("delay")
        .or(Some("3"))
        .unwrap()
        .parse::<u32>()
        .expect("delay must be a valid number");

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
) -> Result<(), Box<Error>> {
    let tmp_dir = TempDir::new()?;
    let tmp_path = tmp_dir.path().to_owned();

    println!("{}", tmp_path.display());

    let found_path = find_profile_folder(profile_folder, profile_name)?;

    let found_name = match found_path {
        None => Err(format!("No profile with name `{}` found", profile_name))?,
        Some((p, name)) => {
            println!("Found profile {} at : {}", name, p.display());

            let options = CopyOptions::new();
            let start = std::time::SystemTime::now();
            // some unique name for new temp profile
            let tmp_dir_name = format!(
                "{}",
                start.duration_since(std::time::UNIX_EPOCH)?.as_millis()
            );
            let new_tmp_path = tmp_dir.path().join(tmp_dir_name);
            dir::create_all(&new_tmp_path, false)?;
            let vec = fs::read_dir(&p)?.map(|x| x.unwrap().path()).collect();
            fs_extra::copy_items(&vec, &new_tmp_path, &options)?;
            new_tmp_path
        }
    };

    let temp_profile_path = Path::new(&tmp_path).join(found_name);

    let command = format!("firefox --profile {}", temp_profile_path.display());

    println!("Command is : {}", command);

    execute_cmd(&command, true, process_restart_delay)?;
    println!("done with first process...");
    execute_cmd(&command, false, process_restart_delay)?;

    tmp_dir.close()?;

    Ok(())
}

fn find_profile_folder<P: AsRef<Path>>(
    profile_folder: P,
    profile_name: &str,
) -> Result<Option<(PathBuf, String)>, Box<Error>> {
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
            println!("{}\t\t->\t{}", entry_profile_name, entry_path.display());
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
) -> Result<(), Box<Error>> {
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
