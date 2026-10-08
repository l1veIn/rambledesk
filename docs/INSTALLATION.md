# Install RambleDesk

[English](INSTALLATION.md) | [简体中文](INSTALLATION.zh-CN.md)

Download the Windows x64 installer or macOS Apple Silicon disk image from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases/latest).

If you are upgrading, read the [data compatibility guide](DATA_COMPATIBILITY.md) first. Back up the complete data directory before trying a release candidate: 0.4.0 cannot open a database upgraded by 0.5.0 directly.

## Windows

Run the downloaded `x64-setup.exe` and follow the installer.

The installer is not yet Authenticode-signed. If SmartScreen blocks it, confirm that the file came from this repository, then choose **More info → Run anyway**.

## macOS

Open the DMG and drag RambleDesk into **Applications**. The build is ad-hoc signed and has not been notarized.

On first launch, right-click RambleDesk and choose **Open**. If it is still blocked, go to **System Settings → Privacy & Security**, find RambleDesk in the **Security** section, choose **Open Anyway**, and confirm **Open**.

If macOS reports that the app is damaged, verify the download source before removing its quarantine flag:

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

## Your first task

1. Open **Settings → Agents**, choose an agent, and follow the prompts to install or connect it. Complete the agent's own sign-in or model setup.
2. Start a new session, choose the agent and a project folder, and enter a task.
3. When the agent asks for feedback, try its result and submit your notes. In sessions started inside RambleDesk, feedback is queued back to the same agent session; its delivery status stays visible.

Text feedback needs no voice or post-processing model. To use voice, download a transcription model in **Settings → Voice** and allow microphone access. Optional AI text processing needs a separately configured model service; the original feedback is preserved.

For connection and recovery details, see the [Agent guide](ACP_MANAGED_SESSIONS.md). To keep using an external agent app or CLI, see [external adapters](COMPATIBILITY.md); Generic MCP requires returning to the agent to continue.
