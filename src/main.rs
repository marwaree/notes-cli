mod config;
use anyhow::{Context, Result, bail};
use clap::Parser;
use config::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

// FIX: editor doesnt start
// TODO: add instructions to edit ~/.gnupg/gpg.conf to add use-agent, configure cache duration with
// ~/.gnupg/gpg-agent.conf default-cache-ttl 28800 and x-cache-ttl 28800, run gpg-connect-agent reloadagent /bye
// TODO: if using github tokens, add instructions to run git config --global credential.helper 'cache --timeout=28800'
// TODO: create or import gpg key in setup
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
    git_pull(&config)?;

    println!("Launching editor...");
    launch_editor(&config)?;

    let message = commit_message_prompt()?;

    println!("Pushing changes to notes repository...");
    git_commit_push(&config, &message)?;
    Ok(())
}

fn git_pull(config: &Config) -> Result<()> {
    let pull_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("pull")
        .arg("--rebase")
        .status()
        .context("Failed to execute git process")?;

    if !pull_output.success() {
        eprintln!("\x1b[1;33mWarning:\x1b[0m Failed to sync repository.",);
        print!("Do you wish to continue? [y/N]: ");

        let mut answer = String::new();

        io::stdout().flush()?;
        io::stdin().read_line(&mut answer)?;

        let answer = answer.trim().to_lowercase();

        match answer.as_str() {
            "yes" | "y" => return Ok(()),
            _ => bail!("Operation aborted by user due to git pull failure"),
        }
    }

    Ok(())
}

fn launch_editor(config: &Config) -> Result<()> {
    let nvim_output = Command::new("nvim")
        .arg(&config.notes_dir)
        .output()
        .context("Failed to execute nvim process")?;

    if !nvim_output.status.success() {
        let stderr = String::from_utf8_lossy(&nvim_output.stderr);
        bail!("{}", stderr.trim());
    }
    Ok(())
}

fn commit_message_prompt() -> Result<String> {
    print!("Commit message [press Enter for default, Esc to cancel sync]: ");
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
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("commit")
        .arg("-m")
        .arg(format!("'{}'", message))
        .output()
        .context("Failed to execute git process")?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        bail!(
            "Failed to commit changes: {}\n\x1b[1;33mWarning:\x1b[0m Remote not synced.",
            stderr.trim()
        );
    }

    match check_remote_encryption(&config) {
        Ok(false) => {
            bail!("Failed to push changes to remote(s): One or more remotes are not encrypted.")
        }
        Err(err) => {
            bail!("Failed to check git remotes encrpytion: {err}")
        }
        Ok(true) => {}
    };

    let push_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("push")
        .status()
        .context("Failed to execute git process")?;

    if !push_output.success() {
        bail!("Failed to push changes to remote.\n\x1b[1;33mWarning:\x1b[0m Remote not synced.",);
    }

    Ok(())
}

fn check_remote_encryption(config: &Config) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .args(["remote", "-v"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(_name), Some(url)) = (parts.next(), parts.next()) {
            if !url.starts_with("gcrypt::") {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

fn setup(config: &mut Config, path: &Path) -> Result<()> {
    print!("Notes directory [~/Notes]: ");
    let mut input = String::new();

    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;

    let raw_path = match input.trim() {
        "" => "~/Notes",
        path => path,
    };

    config.notes_dir = shellexpand::tilde(raw_path).into_owned();

    print!("Git branch [main]: ");
    let mut input = String::new();

    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;

    let branch = match input.trim() {
        "" => "main",
        s => s,
    };

    print!("Git remote: ");
    let mut input = String::new();

    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;

    let remote = input.trim();

    let cryptremote = format!("gcrypt::{}", remote);

    let init_output = Command::new("git")
        .arg("init")
        .arg(&config.notes_dir)
        .output()
        .context("Failed to execute git process")?;

    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr);
        bail!("Failed to create repository: {}", stderr.trim());
    }

    let remote_add_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("remote")
        .arg("add")
        .arg("cryptremote")
        .arg(&cryptremote)
        .output()
        .context("Failed to execute git process")?;

    if !remote_add_output.status.success() {
        let stderr = String::from_utf8_lossy(&remote_add_output.stderr);
        bail!("Failed to add remote to repository: {}", stderr.trim());
    }

    // Initial commit
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("commit")
        .arg("--allow-empty")
        .arg("-m")
        .arg("initial commit")
        .output()
        .context("Failed to execute git process")?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        bail!(
            "Failed to create initial repository commit: {}",
            stderr.trim()
        );
    }

    let branch_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("branch")
        .arg("-M")
        .arg(&branch)
        .output()
        .context("Failed to execute git process")?;

    if !branch_output.status.success() {
        let stderr = String::from_utf8_lossy(&branch_output.stderr);
        bail!("Failed to change branch: {}", stderr.trim());
    }

    println!("Pushing initial commit. Please authenticate below:");
    // Initial push
    let push_output = Command::new("git")
        .arg("-C")
        .arg(&config.notes_dir)
        .arg("push")
        .arg("-u")
        .arg("cryptremote")
        .arg(branch)
        .status()
        .context("Failed to execute git process")?;

    if !push_output.success() {
        bail!("Failed to push initial repository commit.",);
    }

    config.save(path)?;
    Ok(())
}
