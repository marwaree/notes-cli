mod config;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use config::*;
use inquire::validator::Validation;
use inquire::{Confirm, Text};
use std::path::Path;
use std::process::Command;

// TODO: description in readme saying that this is for editing the vault from another editor than
// obsidian. if using obsidian client, use obsidian-git plugin
// TODO: create or import gpg key in setup
// TODO: set up git-remote-gcrypt and gpg keys or import in setup

/// Simple cli wrapper to sync and edit an obsidian vault from an encrypted git remote.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize the encrypted vault repository
    Setup,
    /// Sync local repository without opening the editor
    Pull,
    /// Sync remote with local changes without opening the editor
    Push,
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
    let config_file_path = os_config_dir.join("ogit").join("config.toml");
    let mut config = Config::load_or_create(&config_file_path)?;

    match args.command {
        Some(Commands::Setup) => {
            setup(&mut config, &config_file_path)?;
            return Ok(());
        }
        Some(Commands::Pull) => {
            println!("Syncing vault repository...");
            git_pull(&config)?;
            return Ok(());
        }
        Some(Commands::Push) => {
            // Check if there are any changes
            let status_output = Command::new("git")
                .arg("-C")
                .arg(&config.vault_dir)
                .arg("status")
                .arg("--porcelain")
                .output()
                .context("Failed to check git status")?;

            if status_output.stdout.is_empty() {
                println!("No changes detected. Skipping commit and push.");
                return Ok(());
            }

            let message = get_commit_message(&config)?;

            let push_confirmed = confirm_push()?;

            if push_confirmed {
                git_sync(&config, &message)?;
            } else {
                bail!("Push cancelled by user");
            }

            return Ok(());
        }
        _ => {}
    }

    println!("Syncing vault repository...");
    git_pull(&config)?;

    launch_editor(&config)?;

    // Check if there are any changes
    let status_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
        .arg("status")
        .arg("--porcelain")
        .output()
        .context("Failed to check git status")?;

    if status_output.stdout.is_empty() {
        println!("No changes detected. Skipping commit and push.");
        return Ok(());
    }

    let message = get_commit_message(&config)?;

    let push_confirmed = confirm_push()?;

    if push_confirmed {
        git_sync(&config, &message)?;
    } else {
        bail!("Push cancelled by user\n\x1b[1;33mWarning:\x1b[0m Remote is not synced.");
    }

    Ok(())
}

fn git_pull(config: &Config) -> Result<()> {
    let git_path = Path::new(&config.vault_dir).join(".git");
    if !git_path.is_dir() {
        bail!("Vault git repository doesn't exist. Try `ogit setup` to create it.")
    }

    let fetch_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
        .args(["fetch", "--no-tags", "--quiet"])
        .status()
        .context("Failed to execute git process")?;

    if !fetch_output.success() {
        eprintln!("\x1b[1;33mWarning:\x1b[0m Failed to sync repository.");

        let should_continue = Confirm::new("Do you wish to continue?")
            .with_default(false)
            .prompt_skippable()?;

        match should_continue {
            Some(true) => return Ok(()),
            _ => bail!("Operation aborted by user due to git pull failure"),
        }
    }

    // Check if local HEAD matches the remote upstream branch
    let behind_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
        .args(["rev-list", "--count", "HEAD..@{upstream}"])
        .output()
        .context("Failed to check upstream commits")?;

    if behind_output.status.success() {
        let count = String::from_utf8_lossy(&behind_output.stdout);
        if count.trim() == "0" {
            return Ok(()); // Branch is fully up-to-date
        }
    }
    // Rebase locally only if there are actual upstream changes
    let rebase_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
        .arg("rebase")
        .arg("--autostash")
        .status()
        .context("Failed to execute git process")?;

    if !rebase_output.success() {
        eprintln!("\x1b[1;33mWarning:\x1b[0m Failed to sync repository.");

        let should_continue = Confirm::new("Do you wish to continue?")
            .with_default(false)
            .prompt_skippable()?;

        match should_continue {
            Some(true) => return Ok(()),
            _ => bail!("Operation aborted by user due to git pull failure"),
        }
    }

    Ok(())
}

fn launch_editor(config: &Config) -> Result<()> {
    let editor_str = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());

    let mut args = shell_words::split(&editor_str)
        .with_context(|| format!("Failed to parse $EDITOR command string: '{}'", editor_str))?;

    if args.is_empty() {
        bail!("$EDITOR environment variable was empty");
    }

    let program = args.remove(0);

    let status = Command::new(&program)
        .args(&args)
        .arg(&config.vault_dir)
        .status()
        .with_context(|| format!("Failed to execute editor command: {}", program))?;

    if !status.success() {
        bail!(
            "Editor process ('{}') exited with status: {}",
            program,
            status
        );
    }

    Ok(())
}

fn get_commit_message(config: &Config) -> Result<String> {
    let git_path = Path::new(&config.vault_dir).join(".git");
    if !git_path.is_dir() {
        bail!("Vault git repository doesn't exist. Try `ogit setup` to create it.")
    }

    let message = Text::new("Commit message:")
        .with_default("sync: update notes")
        .with_help_message("Esc to cancel")
        .prompt_skippable()?; // Esc key yields Ok(None)

    match message {
        Some(msg) => Ok(msg),
        None => bail!("Sync cancelled by user"),
    }
}

fn confirm_push() -> Result<bool> {
    let confirmed = Confirm::new("Push changes to remote repository?")
        .with_default(true)
        .prompt()?;

    Ok(confirmed)
}

fn git_sync(config: &Config, message: &str) -> Result<()> {
    let git_path = Path::new(&config.vault_dir).join(".git");
    if !git_path.is_dir() {
        bail!("Vault git repository doesn't exist. Try `ogit setup` to create it.")
    }

    let add_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
        .arg("add")
        .arg("-A")
        .output()
        .context("Failed to execute git process")?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        let stdout = String::from_utf8_lossy(&add_output.stdout);
        let err_msg = if !stderr.trim().is_empty() {
            stderr
        } else {
            stdout
        };

        bail!("Failed to stage files: {}", err_msg.trim());
    }

    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
        .arg("commit")
        .arg("-m")
        .arg(&message)
        .output()
        .context("Failed to execute git process")?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        let stdout = String::from_utf8_lossy(&commit_output.stdout);

        let error_msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            "Unknown Git error (both stdout and stderr were empty)".to_string()
        };

        bail!(
            "Failed to commit changes: {}\n\x1b[1;33mWarning:\x1b[0m Remote not synced.",
            error_msg
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
        .arg(&config.vault_dir)
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
        .arg(&config.vault_dir)
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
    let git_path = format!("{}/.git", config.vault_dir);
    if Path::new(&git_path).is_dir() {
        println!("Already set up at {}. Skipping.", &config.vault_dir);
        return Ok(());
    }

    let raw_path = match Text::new("Vault directory:")
        .with_default("~/Obsidian")
        .with_help_message("Esc to cancel setup")
        .prompt_skippable()?
    {
        Some(s) => s,
        None => bail!("Setup cancelled by user"),
    };

    let vault_dir = shellexpand::tilde(&raw_path).into_owned();

    if !Path::new(&vault_dir).is_dir() {
        bail!("Directory not found. Select an existing obsidian vault or create one.")
    }

    let obsidian_path = format!("{}/.obsidian", vault_dir);

    if !Path::new(&obsidian_path).is_dir() {
        bail!(
            "Directory is not an obsidian vault. Select an existing obsidian vault or create one."
        )
    }

    config.vault_dir = vault_dir;

    let branch = match Text::new("Git branch:")
        .with_default("main")
        .with_help_message("Esc to cancel setup")
        .prompt_skippable()?
    {
        Some(s) => s,
        None => bail!("Setup cancelled by user"),
    };

    let remote = match Text::new("Git remote:")
        .with_help_message("Required. Press Esc to cancel setup.")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                Ok(Validation::Invalid(
                    "Git remote URL cannot be empty.".into(),
                ))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt_skippable()?
    {
        Some(input) => input.trim().to_string(),
        None => bail!("Setup cancelled by user"),
    };

    let cryptremote = format!("gcrypt::{}", remote);

    let init_output = Command::new("git")
        .arg("init")
        .arg(&config.vault_dir)
        .output()
        .context("Failed to execute git process")?;

    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr);
        bail!("Failed to create repository: {}", stderr.trim());
    }

    let remote_add_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
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
        .arg(&config.vault_dir)
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
        .arg(&config.vault_dir)
        .arg("branch")
        .arg("-M")
        .arg(&branch)
        .output()
        .context("Failed to execute git process")?;

    if !branch_output.status.success() {
        let stderr = String::from_utf8_lossy(&branch_output.stderr);
        bail!("Failed to change branch: {}", stderr.trim());
    }

    println!("Pushing initial commit. Authentication may be required.");
    // Initial push
    let push_output = Command::new("git")
        .arg("-C")
        .arg(&config.vault_dir)
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
