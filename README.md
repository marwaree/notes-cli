# notes-cli

## Simple cli utility to sync notes from an encrypted git remote.

Setup encrypted git repository:

```sh
notes-cli setup
```

Pull latest changes and edit local directory with default editor.
This will ask you for a commit message and push on exit:

```sh
notes-cli
```

Pull latest changes without opening the editor:

```sh
notes-cli pull
```

Push changes without opening the editor.
This will ask you for a commit message:

```sh
notes-cli push
```

## Requirements

```sh
yay -S git-remote-gcrypt
```
