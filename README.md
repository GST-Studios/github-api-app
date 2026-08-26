# GitHub API App

A read-only Rust CLI client for `github-api`.

## Configure

Set these environment variables:

```powershell
$env:API_BASE_URL = "http://127.0.0.1:3000"
$env:API_KEY = "your-api-key"
```

The API server must already be running, and the GitHub username must have connected through OAuth with private repository permission.

## Run

List every accessible repository name and link:

```powershell
cargo run -- MICKYcyber
```

List every repository plus all file/folder names and links for one repository:

```powershell
cargo run -- MICKYcyber your-repository
```
