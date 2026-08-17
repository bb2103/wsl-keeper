<p align="center">
  <img src="src-tauri/icons/128x128.png" width="80" height="80" alt="WSL Keeper">
</p>

<h1 align="center">WSL Keeper</h1>

<p align="center">
  <b>English</b> ·
  <a href="docs/zh-CN/README.md">简体中文</a>
</p>

<p align="center">
  <a href="https://github.com/bb2103/wsl-keeper/actions/workflows/ci.yml"><img src="https://github.com/bb2103/wsl-keeper/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/bb2103/wsl-keeper/releases/latest"><img src="https://img.shields.io/github/v/release/bb2103/wsl-keeper?include_prereleases" alt="release"></a>
</p>

Keeps a WSL distro running. Remounts Linux disks after reboot.

Close the window → tray. Quit from the tray menu.

<p align="center">
  <img src="docs/interface-en_us.png" alt="Dashboard" width="720">
</p>

## Install

- Windows 10/11 x64, [WSL](https://learn.microsoft.com/windows/wsl/install) installed
- Disk mount needs **WSL 2**
- [Releases](https://github.com/bb2103/wsl-keeper/releases) → `-setup.exe` (or `.msi`)

## Usage

1. **Settings** → pick a distro → turn the guardian on
2. Optional: a command to run after the distro starts
3. Linux disk (ext4 / xfs / btrfs) → Disks → Guard → auto-mount → accept the one admin prompt  
   Mount point: `/mnt/wsl/<name>`
4. Pause from the dashboard or tray when you don’t want auto-restart
