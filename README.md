# WSL Keeper

Windows tray app that keeps a WSL distro running and remounts Linux physical disks after reboot.

Close the window to hide to the tray. Quit only from the tray menu.

## Layout

```
src/                      React UI
  api/                    IPC client (`keeper.*`)
  lib/                    shared hooks and formatters
  screens/                Dashboard, Settings
src-tauri/src/
  domain/                 config, state, actions, status
  platform/               WSL, disks, elevated mount task
  runtime/                guardians, tray, log, notify
  ipc/                    Tauri commands
```

Config and logs live in `%APPDATA%\lea\wsl-keeper`.

## Develop

```
npm install
npm run tauri dev
```

Needs Visual Studio Build Tools (MSVC `link.exe`) for the default Windows toolchain.
