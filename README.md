# PS2 Textures Downloader

A cross-platform desktop app for PS2 mod projects that helps users download and keep up-to-date large texture replacement packs. Built with Tauri (Rust + React).

## Table of Contents
- [Features](#features)
  - [Mod Installer](#introduction--installer)
  - [Mod Updater](#introduction--updater)
  - [Post-Sync Verification](#introduction--verification)
- [Handling User-Custom Textures](#custom-textures)
- [Installation](#installation)
  - [Windows](#installation--windows)
  - [macOS](#installation--macos)
- [Using the App](#usage)
  - [First Time Setup](#usage--setup)
  - [Updating and Syncing](#usage--sync)
- [For Mod Teams: Customizing for Your Project](#for-mod-teams)
- [License](#license)

---

## Features <a name="features">

For users of a PS2 mod that requires a massive folder of replacement textures, downloading multi-GB zip files and keeping things updated can be tedious. This app provides:

### Mod Installer <a name="introduction--installer">

The **First Time Setup** uses Git sparse checkout to efficiently download only the texture files you need (not the entire repository). This is faster and more reliable than downloading a massive zip file, which can fail or become corrupted. The installer automatically places textures in the correct location within your emulator's textures folder.

### Mod Updater <a name="introduction--updater">

The **Sync** feature keeps your textures up-to-date with two modes:

- **Download New Content** (Incremental Sync): Quickly grabs only the changes since your last sync. Uses the GitHub Compare API to identify new, modified, renamed, and deleted files.

- **Full Sync**: Compares every local file against the repository using SHA hash verification. Use this occasionally or when experiencing texture issues.

Both modes will:
- Download new and modified files
- Rename/move files that were reorganized
- Delete files that were removed from the project
- Preserve your disabled textures (dash-prefixed files)
- Never touch your `user-customs` folder

### Post-Sync Verification <a name="introduction--verification">

After every sync, the app performs a quick file count verification to ensure your local installation matches the repository. If a mismatch is detected, you'll be prompted to run a Full Sync to resolve discrepancies.

---

## Handling User-Custom Textures <a name="custom-textures">

This app is designed with texture customization in mind:

### The `user-customs` Folder

Put all of your custom textures in the `user-customs` folder (inside the `replacements` folder). **The app will never modify, update, or delete anything in this folder.** This is the safe place for your personal textures and DLC content.

### Disabling Default Textures

When using custom textures, you need to disable the mod's default texture so yours takes precedence. To do this:

1. **Keep the default texture in place** (don't delete it)
2. **Prepend the filename with a dash** (e.g., rename `3a30272f374c5d47.png` to `-3a30272f374c5d47.png`)

The dash prefix "disables" the texture - the emulator ignores it, but the app still recognizes it. When the mod team updates that texture, **your disabled version will be updated too**, keeping you in sync without breaking your custom texture.

**Important**: If you delete the default texture instead of disabling it, the sync will re-download it and potentially cause conflicts with your custom texture.

---

## Installation <a name="installation"></a>

### Windows <a name="installation--windows"></a>

1. Download `windows-portable.zip` from the [latest release](../../releases/latest)
2. Extract the zip file somewhere on your computer (e.g., `C:\Apps\` or your Desktop)
3. Open the extracted folder and run the `.exe` file to launch the app

**Note**: The app includes a bundled copy of Git (MinGit), so you don't need to install Git separately.

#### Updating the App (Windows)

1. Download the new `windows-portable.zip` from the latest release
2. Extract and replace the existing app folder
3. Your settings (including GitHub API token) are stored separately and will be preserved

### macOS <a name="installation--macos"></a>

1. Download the Mac installer file from the [latest release](../../releases/latest)
2. Open the DMG and drag the app to your Applications folder
3. On first launch, right-click the app and select "Open" to bypass Gatekeeper. In some cases you might need to go to Setting > Privacy & Security, scroll down, and allow the app to run in the Security settings section.

#### Updating the App (macOS)

Simply download the new `.dmg` and drag the app to your Applications folder, replacing the old version. Your settings are stored in your user Library folder and will be preserved.

---

## Using the App <a name="usage"></a>

### First Time Textures Installation <a name="usage--setup">

1. Select the **Install** tab
2. Browse to your PCSX2 textures folder. You can find the exact path in PCSX2 (or AetherSX2) at Settings > Graphics > Texture Replacements.
3. Click **Start Installation**
4. Wait for the download to complete (this may take a while for large texture packs)

The installer uses Git sparse checkout to efficiently download only the texture files. Progress is displayed in real-time. 

**Requirements for Mac Users Only**: Git must be installed. If you don't have it, install Xcode Command Line Tools by running in Terminal:
```bash
xcode-select --install
```

### Updating and Syncing <a name="usage--sync">

1. Select the **Sync** tab
2. Ensure your GitHub API Token is configured (instructions below)
3. Choose your sync mode:
   - **Download New Content**: Fast, only downloads changes since last sync (recommended for regular use)
   - **Full Sync**: Compares all files, slower but thorough (use occasionally or when troubleshooting)
4. Click **Run Sync**

**Warning Dialogs**: When running a Full Sync, if files will be replaced or deleted, you'll see a warning dialog listing the affected files. This gives you a chance to back up any custom textures to the `user-customs` folder before proceeding.

#### GitHub API Token (Required for Sync)

A GitHub Personal Access Token is required for the sync features. Here's how to get one:

1. Create a free Github account, if needed, and generate a "Fine-Grained" API token. Go to Settings > Developer Settings > Personal Access Tokens > Fine-Grained Tokens > [Generate New Token](https://github.com/settings/personal-access-tokens/new?name=Textures+Downloader&description=Token+for+syncing+textures&expires_in=365).
2. Give it a name (e.g., "PS2 Textures Downloader")
3. Set expiration to 1 year (maximum)
4. **No permissions are needed** - leave everything unchecked
5. Click "Generate Token" and copy it
6. Paste the token into the app's GitHub API Token field and click Save.

---

## For Mod Teams: Customizing for Your Project <a name="for-mod-teams"></a>

This app is open-source and can be customized for any PS2 texture replacement mod. Fork the repository and modify the configuration files for your project before building your apps.

### Configuration Files

You need to update two configuration files:

#### Backend: `src-tauri/src/config.rs`

```rust
/// Repository owner (GitHub username or organization)
pub const REPO_OWNER: &str = "your-github-username";

/// Name of the texture mod repository
pub const REPO_NAME: &str = "your-repo-name";

/// Full URL to the git repository
pub const REPO_URL: &str = "https://github.com/your-username/your-repo-name.git";

/// The target folder name (typically the PS2 game identifier)
pub const SLUS_FOLDER: &str = "SLUS-XXXXX";

/// Path within the repo to sparse checkout
pub const SPARSE_PATH: &str = "textures/SLUS-XXXXX";
```

#### Frontend: `frontend/config.ts`

```typescript
/// Application title displayed in the header
export const APP_TITLE = "Your Mod Name Textures Downloader";

/// Repository owner (GitHub username or organization)
export const REPO_OWNER = "your-github-username";

/// Repository name
export const REPO_NAME = "your-repo-name";

/// The target folder name
export const TARGET_FOLDER = "SLUS-XXXXX";

/// Path within the repo to sparse checkout
export const SPARSE_PATH = "textures/SLUS-XXXXX";
```

#### App Metadata: `src-tauri/tauri.conf.json`

Update the app identifier, title, and other metadata:

```json
{
  "productName": "Your Mod Textures Downloader",
  "identifier": "com.yourteam.textures-downloader",
  ...
}
```

### Repository Structure

Your texture repository should be structured as follows:

```
your-repo/
└── installer-data.json
└── textures/
    └── SLUS-XXXXX/
        └── replacements/
            ├── user-customs/     <- Users put custom textures here (never modified by sync)
            └── ...
```

### User-Customs Folder

Ensure your repository has a `user-customs` folder in the replacements directory. This folder should exist (can contain a `.gitkeep` file if there are no other files in it) so users have a designated safe space for their custom textures.

### Installer JSON file

Ensure your repository has a file called `installer-data.json` at the root of the repo. It should contain the following:

```json
{
  "release_url": "https://github.com/myteam/my-project",
  "total_size": 22.5,
  "min_downloader_app_version": 0.25,
  "downloader_app_url": "http://textures.myproject.com"
}
```
The `total_size` is the approximate size of your textures folder in GB. This is used to inform the user about the required disk space before initial installation.

The `min_downloader_app_version` is checked when the app runs. If the app's version is lower than the required version, it will block the user from using the app and prompt them to download the latest version from the `downloader_app_url` URL. This is useful for restricting use of outdated and/or bugged versions of the app.

### Building the App

Prerequisites:
- Node.js 20+
- Rust (install via [rustup.rs](https://rustup.rs))
- Platform-specific dependencies (see [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites))

```bash
# Install dependencies
npm install

# Development
npm run tauri dev

# Build for release
npm run tauri build
```

### GitHub Actions

The repository includes GitHub Actions workflows for automated builds. On each push to `main`, it builds:
- Windows: Portable `.exe` + `resources` folder
- macOS: `.dmg` installer (universal binary for Intel and Apple Silicon)

Build artifacts are attached to each workflow run and can be downloaded from the Actions tab.

### Bundling MinGit for Windows (Required)

The Windows portable build requires MinGit to be manually bundled due to a Tauri resource bundling limitation. After each build:

1. **Download build artifacts** from GitHub Actions:
   - Download `windows-portable` artifact (contains the `.exe` and `resources` folder) from your repo's Github workflow (the Actions tab)

2. Create a folder that will become the root of the new windows-portable zip (Eg. My Project Textures Downloader)

3. Put the `.exe` and `resources` folder into the folder

4. **Add MinGit**:
   - Copy the mingit folder from this project's `src-tauri` folder (or get MinGit from [git-for-windows/git releases](https://github.com/git-for-windows/git/releases))
   - Paste it into the `resources` folder so you have:
     ```
     myproject-textures-downloader.exe
     resources/
     ├── icon.ico
     └── mingit/
         └── x64/
             ├── cmd/
             │   └── git.exe    <- This is what the app looks for
             ├── etc/
             ├── mingw64/
             ├── usr/
             └── LICENSE.txt
     ```

5. **Create the release zip**:
   - Zip the folder you created in step 2
   - Name it `windows-portable.zip`
   - Attach to your GitHub release

**Note**: The app looks for Git at `resources/mingit/x64/cmd/git.exe`. If this path doesn't exist, users will see an error asking them to install Git manually.

### Syncing Your Fork with Upstream

To pull bug fixes and new features from the upstream repository into your fork:

```bash
# Add upstream remote (one-time setup)
git remote add upstream https://github.com/jd6-37/ps2-textures-downloader.git

# Fetch and merge upstream changes
git fetch upstream
git merge upstream/main
```

If upstream has modified the config files, you'll get merge conflicts. Simply resolve them by keeping your project's values:

1. Open the conflicted config file(s)
2. Keep your customized values (reject the upstream template values)
3. Complete the merge: `git add . && git commit`

Config files are small and rarely change, so conflicts are infrequent and easy to resolve.

---

## License <a name="license"></a>

PS2 Textures Downloader © 2024-2026 by JD6-37 is licensed under [CC BY-NC 4.0](http://creativecommons.org/licenses/by-nc/4.0/)

This license requires that reusers give credit to the creator. It allows reusers to distribute, remix, adapt, and build upon the material in any medium or format, for noncommercial purposes only.
