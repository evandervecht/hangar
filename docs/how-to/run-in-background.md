# Run Hangar in the background

Hangar can keep running in the system tray (Windows) or menu bar (macOS) and rescan on a timer, so the list is up to date whenever you open it.

## Keep running when the window is closed

1. Click **Settings**.
2. Under **Background**, tick **Keep running in the tray when the window is closed**.
3. Set **Rescan in the background every … minutes** (default 15).
4. Click **Save and rescan**.

Closing the window now hides it. Click the Hangar icon in the tray to open it again, or right-click it for **Open Hangar**, **Rescan now** and **Quit Hangar**.

## Start at login

In **Settings**, tick **Start Hangar in the tray when I log in** and save. This also turns on background mode. Hangar then starts hidden in the tray when you log in.

- **Windows:** adds a `Hangar` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- **macOS:** adds `~/Library/LaunchAgents/io.github.evandervecht.hangar.plist`.
- **Linux:** adds `~/.config/autostart/hangar.desktop`.

Untick it and save to remove the entry again. The entry points at the Hangar program you're running, so set it from your installed copy, not from a build folder.

## Open the window from a terminal

Run `hangar` while Hangar is already running: the running copy shows its window and the new one exits. Only one Hangar window runs per user.

## Linux

There's no tray icon on Linux, so closing the window quits Hangar as usual. While the window is open, background mode still rescans on the timer.
