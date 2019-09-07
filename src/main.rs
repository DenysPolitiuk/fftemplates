extern crate clap;
extern crate dirs;
extern crate fs_extra;
extern crate tempfile;

use clap::App;
use clap::Arg;
use fs_extra::dir;
use fs_extra::dir::CopyOptions;
use regex::Captures;
use regex::Regex;
use tempfile::TempDir;

use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time;
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

const EXTENSIONS_JSON: &str = "extensions.json";

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
        .get_matches();

    let profile_name = matches
        .value_of("base_profile")
        .or(Some("default"))
        .unwrap();

    let profile_folder = Path::new(&dirs::home_dir().unwrap())
        .join(Path::new(".mozilla"))
        .join(Path::new("firefox"));

    if let Err(e) = run(profile_folder, profile_name) {
        println!("Error from run : {}", e);
    }
}

fn run<P: AsRef<Path>>(profile_folder: P, profile_name: &str) -> Result<(), Box<dyn Error>> {
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
    let extensions = new_tmp_path.join(Path::new(EXTENSIONS_JSON));
    if extensions.exists() {
        if let Err(e) = adjust_extensions_json(&extensions) {
            Err(format!("Error during adjusting extensions json : {}", e))?;
        }
    }

    let command = format!("firefox --profile {}", new_tmp_path.display());

    execute_cmd(&command)?;

    tmp_dir.close()?;

    Ok(())
}

fn adjust_extensions_json(extensions: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut content = String::new();
    {
        let file = File::open(extensions)?;
        let mut buf_reader = BufReader::new(file);
        buf_reader.read_to_string(&mut content)?;
    }

    let mut temp_path = Path::new(extensions).to_path_buf();
    temp_path.pop();
    let re = Regex::new(
        r#"(?x)
    ("path":)                       # starting with "path":
    (")(                              # "
    ([\w\W--"]+)                    # any number of words or characters except for " to avoid going over the value in json
    (extensions[\w\W--"]+\.xpi)     # trying to match ending of extenstions/some.extension@name.xpi
    )(")                              # "
    "#,
    )?;

    let results = re.replace_all(content.as_str(), |caps: &Captures| {
        format!(
            "{}{}{}{}",
            &caps[1],
            &caps[2],
            temp_path.join(&caps[5]).display(),
            &caps[6]
        )
    });

    {
        let file = File::create(extensions)?;
        let mut buf_writer = BufWriter::new(file);
        buf_writer.write_all(results.as_bytes())?;
    }

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

pub fn execute_cmd(cmd: &String) -> Result<(), Box<dyn Error>> {
    let cmd_split: Vec<_> = cmd.split(' ').collect();
    if cmd_split.len() < 1 || cmd_split[0] == "" {
        return Err("No command specified")?;
    }

    let proc;
    if cmd_split.len() < 2 {
        proc = Command::new(cmd_split[0]).spawn()?;
    } else {
        proc = Command::new(cmd_split[0])
            .args(&cmd_split[1..cmd_split.len()])
            .spawn()?;
    }

    let _ = proc.wait_with_output()?;

    Ok(())
}
