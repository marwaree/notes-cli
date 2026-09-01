mod config;
use anyhow::{Context, Result, bail};
use clap::Parser;
use config::*;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

// TODO: github sync - check if EVERY remote url is gcrypt:: before pushing unless there is no remote (local repo)
// TODO: option for local repo on setup
// TODO: set up git-remote-gcrypt and gpg keys or import in setup

/// Simple cli utility to sync notes from an encrypted git remote.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    command: Option<String>,
}

fn main() {
    let args = Args::parse();

    if let Err(err) = run(args) {
        eprintln!("\x1b[1;31mError:\x1b[0m {err:#}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<()> {
    let os_config_dir =
        dirs::config_dir().context("Cannot find config directory on this system")?;
    let config_file_path = os_config_dir.join("notes-cli").join("config.toml");
    let mut config = Config::load_or_create(&config_file_path)?;

    match args.command.as_deref() {
        Some("setup") => {
            setup(&mut config, &config_file_path)?;
            return Ok(());
        }
        _ => {}
    }

    let git_path = format!("{}/.git", config.notes_dir);
    if !Path::new(&git_path).is_dir() {
        bail!("Notes git repository doesn't exist. Try `notes-cli setup` to create it.")
    }

    // println!("Syncing notes repository...");
    // git_pull().context("Failed during initial git pull")?;

    println!("Opening Neovim...");
    launch_editor(&config).context("Failed while running editor")?;

    // println!("Processing post-edit sync...");
    // prompt_and_push().context("Failed during post-edit git sync")?;

    // println!("Notes synced successfully.");
    Ok(())
}

fn launch_editor(config: &Config) -> Result<()> {
    let nvim_status = Command::new("nvim").arg(&config.notes_dir).status()?;
    if !nvim_status.success() {
        bail!("Neovim exited with an error");
    }
    Ok(())
}

fn setup(config: &mut Config, path: &Path) -> Result<()> {
    let mut notes_dir = String::new();

    print!("Notes directory [~/Notes]: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut notes_dir)?;

    let trimmed = notes_dir.trim();

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

    let init_status = Command::new("git")
        .arg("init")
        .arg(&config.notes_dir)
        .status()
        .with_context(|| format!("Failed to execute 'git init' in {}", &config.notes_dir))?;

    if !init_status.success() {
        bail!(
            "'git init' failed to create repository at {}",
            &config.notes_dir
        );
    }

    let _ = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("remote")
        .arg("add")
        .arg("cryptremote")
        .arg(&cryptremote)
        .status()
        .with_context(|| {
            format!(
                "Failed add remote to repository: {}. Do you have git-remote-gcrypt installed?",
                &config.notes_dir
            )
        });

    config.save(path)?;
    Ok(())
}
