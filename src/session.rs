use regex::Captures;
use regex::Regex;

use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::Path;

const PROFILE_FILE_NAME: &'static str = "prefs.js";
const SESSIONSTORE_DEFAULT_NAME: &'static str = "sessionstore.jsonlz4";

pub fn adjust_profile_settings(
    folder_location: &str,
    disable_clean_history_on_close: bool,
) -> Result<(), Box<dyn Error>> {
    let preferences = Path::new(folder_location).join(Path::new(PROFILE_FILE_NAME));
    let mut content = String::new();
    {
        let file = File::open(&preferences)?;
        let mut buf_reader = BufReader::new(file);
        buf_reader.read_to_string(&mut content)?;
    }

    // enable saving history
    let re = Regex::new(r#"(user_pref)(\("places.history.enabled", )(false|true)(\);)"#)?;
    content = re
        .replace_all(content.as_str(), |caps: &Captures| {
            format!("{}{}{}{}", &caps[1], &caps[2], "true", &caps[4])
        })
        .into_owned();

    // enable saving session
    let re = Regex::new(r#"user_pref\("browser.startup.page", (\d)\);"#)?;
    // expected behaviour
    if !re.is_match(&content) {
        content.push_str(r#"user_pref("browser.startup.page", 3);"#);
    }

    // disable history sanitization on closing (needed to store session)
    if disable_clean_history_on_close {
        let re = Regex::new(
            r#"(user_pref)(\("privacy.sanitize.sanitizeOnShutdown", )(false|true)(\);)"#,
        )?;
        content = re
            .replace_all(content.as_str(), |caps: &Captures| {
                format!("{}{}{}{}", &caps[1], &caps[2], "false", &caps[4])
            })
            .into_owned();
    }

    {
        let file = File::create(&preferences)?;
        let mut buf_writer = BufWriter::new(file);
        buf_writer.write_all(content.as_bytes())?;
    }

    Ok(())
}

pub fn add_sessionstore_file(
    file_location: &str,
    folder_location: &str,
    fail_if_does_not_exist: bool,
) -> Result<(), Box<dyn Error>> {
    let sessionstore = Path::new(file_location);
    if !sessionstore.exists() && fail_if_does_not_exist {
        Err(format!(
            "`{}` sessionstore file doesn't exist",
            file_location
        ))?;
    } else if !sessionstore.exists() && !fail_if_does_not_exist {
        return Ok(());
    }

    fs::copy(
        sessionstore,
        Path::new(folder_location).join(Path::new(SESSIONSTORE_DEFAULT_NAME)),
    )?;

    Ok(())
}

pub fn save_sessionstore_file(
    file_name: &str,
    folder_location: &str,
) -> Result<(), Box<dyn Error>> {
    let sessionstore = Path::new(file_name);
    let source_session_store =
        Path::new(folder_location).join(Path::new(SESSIONSTORE_DEFAULT_NAME));

    fs::copy(source_session_store, sessionstore)?;

    Ok(())
}
