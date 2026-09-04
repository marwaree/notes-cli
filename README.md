# obsidian-git-cli

## Simple cli wrapper to sync and edit an obsidian vault from an encrypted git remote

### Requirements

```sh
yay -S git-remote-gcrypt
```

### Quickstart

Setup encrypted git repository:

```sh
ogit setup
```

Pull latest changes and edit local directory with default editor.
This will ask you for a commit message and push on exit:

```sh
ogit
```

Pull latest changes without opening the editor:

```sh
ogit pull
```

Push changes without opening the editor.
This will ask you for a commit message:

```sh
ogit push
```

### Configuration

It is recommended to cache git and gpg credentials to avoid having to enter them over and over.
You can do this by editing the following files:

#### ~/.gnupg/gpg.conf

```conf
use-agent
```

#### ~/.gnupg/gpg-agent.conf

```conf
default-cache-ttl 28800
x-cache-ttl 28800
```

Then reload the agent to apply the new configuration:

```sh
gpg-connect-agent reloadagent /bye
```

To do the same for git credentials, run the following command:

```sh
git config --global credential.helper 'cache --timeout=28800'
```
