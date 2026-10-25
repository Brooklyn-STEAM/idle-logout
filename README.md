# Idle Logout

Signs Windows users out when they walk away. After 1 minute with no keyboard or mouse input, a panel slides up from the bottom of the screen with a countdown. Users can choose to keep working or to signout. If no option is chosen, the user is signed out when the countdown ends.

A clock icon in the system tray shows the app is running. Hover over it to see the version.

## Install

Download `idle-logout-setup.exe` from the latest release and run it as an administrator. It picks the x64 or ARM64 build for the PC and starts the app at sign-in for every user.

## Settings

Set these DWORD values, in seconds, under `HKEY_LOCAL_MACHINE\Software\Idle Logout`. The app reads them when it starts, so changes apply at the next sign-in.

| Value | Default | Sets |
|---|---|---|
| `IdleSeconds` | 60 | Idle time before the panel appears |
| `CountdownSeconds` | 240 | Length of the countdown |

## Development

The Rust version is pinned in `rust-toolchain.toml`.

```
cargo run -- --dry-run
cargo test
```

`--dry-run` uses a 5-second idle time and a 10-second countdown, and prints a message instead of signing out. On Linux and macOS the panel works for previewing, but idle time counts from launch and there is no tray icon.
