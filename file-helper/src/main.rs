use std::io::Write;
use takedock_files::{Identity, Root, VIDEO_ROOT};
fn run() -> Result<(), String> {
    let root = Root::open(std::path::Path::new(VIDEO_ROOT))?;
    let _lock = root.lock()?;
    root.recover()?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["list"] => {
            for entry in root.list()? {
                let mut output = std::io::stdout().lock();
                output
                    .write_all(entry.name.as_bytes())
                    .map_err(|error| error.to_string())?;
                output.write_all(b"\0").map_err(|error| error.to_string())?;
                output
                    .write_all(serde_json::to_string(&entry).unwrap().as_bytes())
                    .map_err(|error| error.to_string())?;
                output.write_all(b"\0").map_err(|error| error.to_string())?;
            }
        }
        ["stat", name] => println!("{}", serde_json::to_string(&root.stat(name)?).unwrap()),
        ["hash", name, expected] => {
            if !root.video_ready(name)? {
                return Err("video_not_finalized".into());
            }
            let expected: Identity =
                serde_json::from_str(expected).map_err(|error| error.to_string())?;
            println!("{}", root.hash(name, &expected)?);
        }
        ["read", name, expected] => {
            if !root.video_ready(name)? {
                return Err("video_not_finalized".into());
            }
            let expected: Identity =
                serde_json::from_str(expected).map_err(|error| error.to_string())?;
            root.read(name, &expected, &mut std::io::stdout().lock())?;
        }
        ["rename", name, new_name, expected] => {
            if !root.video_ready(name)? {
                return Err("video_not_finalized".into());
            }
            let expected: Identity =
                serde_json::from_str(expected).map_err(|error| error.to_string())?;
            root.rename(name, new_name, &expected)?;
        }
        ["delete", name, expected] => {
            if !root.video_ready(name)? {
                return Err("video_not_finalized".into());
            }
            let expected: Identity =
                serde_json::from_str(expected).map_err(|error| error.to_string())?;
            root.delete(name, &expected)?;
        }
        _ => return Err("invalid_helper_command".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
