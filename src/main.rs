mod config;
use anyhow::{Context, Result, bail};
use clap::Parser;
use config::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

// TODO: Ask for branch during setup
// TODO: github sync - check if EVERY remote url is gcrypt:: before pushing unless there is no remote (local repo)
// TODO: option for local repo on setup
// TODO: set up git-remote-gcrypt and gpg keys or import in setup
// TODO: add support for other editors

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

    println!("Syncing notes repository...");
    git_pull(&config).context("Failed during initial git pull")?;

    launch_editor(&config).context("Failed while running editor")?;

    let message = commit_message_prompt()?;

    println!("Pushing changes to notes repository...");
    git_commit_push(&config, &message)?;
    Ok(())
}

fn git_pull(config: &Config) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("pull")
        .arg("--rebase")
        .status()?;
    if !status.success() {
        bail!("Failed to pull repository");
    }
    Ok(())
}

fn launch_editor(config: &Config) -> Result<()> {
    let status = Command::new("nvim").arg(&config.notes_dir).status()?;
    if !status.success() {
        bail!("Neovim exited with an error");
    }
    Ok(())
}

fn commit_message_prompt() -> Result<String> {
    print!("Commit message [press Enter for default, Esc to cancel]: ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();

    loop {
        if let Event::Key(key_event) = event::read().context("Failed to read terminal event")? {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }

            match key_event.code {
                KeyCode::Esc => {
                    println!();
                    bail!("Sync cancelled by user");
                }

                KeyCode::Enter => {
                    println!();
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        return Ok("sync: update notes".to_string());
                    } else {
                        return Ok(trimmed.to_string());
                    }
                }

                KeyCode::Backspace => {
                    if input.pop().is_some() {
                        print!("\x08 \x08");
                        io::stdout().flush()?;
                    }
                }

                KeyCode::Char(c) => {
                    input.push(c);
                    print!("{c}");
                    io::stdout().flush()?;
                }

                _ => {}
            }
        }
    }
}

fn git_commit_push(config: &Config, message: &str) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("commit")
        .arg("-m")
        .arg(format!("'{}'", message))
        .status()?;
    if !status.success() {
        bail!("Failed to commit changes.\n\x1b[1;33mWarning:\x1b[0m Remote not synced.");
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("push")
        .status()?;
    if !status.success() {
        bail!("Failed to push changes to remote.\n\x1b[1;33mWarning:\x1b[0m Remote not synced.");
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

    // Initial commit
    let commit_status = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("commit")
        .arg("--allow-empty")
        .arg("-m")
        .arg("initial commit")
        .status()
        .context("Failed to create initial commit")?;

    if !commit_status.success() {
        bail!("Failed to create initial repository commit");
    }

    // Initial push
    let push_status = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("push")
        .arg("-u")
        .arg("cryptremote")
        .arg("main")
        .status()
        .context("Failed initial push to cryptremote")?;

    if !push_status.success() {
        bail!("Failed to push initial commit to remote repository");
    }

    config.save(path)?;
    Ok(())
}
