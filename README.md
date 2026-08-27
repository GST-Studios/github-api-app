# GitHub API App

A native read-only Rust GUI client for `github-api`.

## Configure

Set these environment variables:

```powershell
$env:API_BASE_URL = "http://127.0.0.1:3000"
$env:API_KEY = "your-api-key"
```

The API server must already be running, and the GitHub username must have connected through OAuth with private repository permission.

## Run the GUI

Start the desktop app:

```powershell
cargo run
```

Enter the API URL, API key, GitHub username or organization name, and repository name in the window. Enable **Owner is organization** for `GST-Studios`. Use **Load repositories** to display every accessible repository name and link, or **Load contents** to display every file/folder name and link for the selected repository.

The app also reads initial values from these environment variables:

```powershell
$env:API_BASE_URL = "http://127.0.0.1:3000"
$env:API_KEY = "your-api-key"
$env:GITHUB_USERNAME = "MICKYcyber"
```
