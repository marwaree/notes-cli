mod config;
use clap::Parser;
use config::*;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// TODO: error when notes is executed without config or dir
// TODO: github sync

/// Simple cli utility to sync notes from an encrypted git remote.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    command: Option<String>,
}

fn main() {
    let os_config_dir = dirs::config_dir().expect("Cannot find config directory on this system");
    let config_file_path = os_config_dir.join("notes-cli").join("config.toml");

    let mut config = Config::load_or_create(&config_file_path)
        .expect("Failed to open or create configuration file");

    let args = Args::parse();

    match args.command.as_deref() {
        Some("setup") => setup(&mut config, &config_file_path).expect("Failed to set up"),
        _ => edit(&config.notes_dir).expect("Failed to edit notes"),
    }
}

fn edit(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _ = Command::new("nvim").arg(path).status()?;
    Ok(())
}

fn setup(config: &mut Config, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut notes_dir = String::new();

    print!("Notes directory [~/Notes]: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut notes_dir)?;

    let trimmed = notes_dir.trim();

    // 2. Fall back to "~/Notes" if the user pressed Enter on an empty line
    let chosen_path = if trimmed.is_empty() {
        "~/Notes"
    } else {
        trimmed
    };

    let expanded_dir = shellexpand::tilde(chosen_path).to_string();

    config.notes_dir = expanded_dir;

    let mut remote = String::new();

    print!("Git remote: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut remote)?;

    let trimmed = remote.trim();

    let cryptremote = format!("gcrypt::{}", trimmed);

    let _ = Command::new("git")
        .arg("init")
        .arg(&config.notes_dir)
        .status()?;

    let _ = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("remote")
        .arg("add")
        .arg("cryptremote")
        .arg(&cryptremote)
        .status()?;

    config.save(path)?;
    Ok(())
}
