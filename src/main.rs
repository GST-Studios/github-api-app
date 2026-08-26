use reqwest::{Client, Url};
use serde::Deserialize;
use std::{env, error::Error};

type AppResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Deserialize)]
struct LinkItem {
    name: String,
    link: String,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let mut arguments = env::args().skip(1);
    let username = arguments
        .next()
        .ok_or("usage: github-api-app <github-username> [repository]")?;
    let selected_repository = arguments.next();

    let base_url = env::var("API_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_owned());
    let api_key = env::var("API_KEY").map_err(|_| "API_KEY must be set")?;
    let client = Client::new();

    let repositories = get_links(
        &client,
        &api_key,
        &base_url,
        &["v1", "users", &username, "repositories"],
    )
    .await?;

    println!("Repositories for {username}:");
    for repository in &repositories {
        println!("{} -> {}", repository.name, repository.link);
    }

    if let Some(repository) = selected_repository {
        let tree = get_links(
            &client,
            &api_key,
            &base_url,
            &["v1", "users", &username, "repos", &repository, "tree"],
        )
        .await?;

        println!("\nContents of {username}/{repository}:");
        for item in tree {
            println!("{} -> {}", item.name, item.link);
        }
    }

    Ok(())
}

async fn get_links(
    client: &Client,
    api_key: &str,
    base_url: &str,
    path_segments: &[&str],
) -> AppResult<Vec<LinkItem>> {
    let mut url = Url::parse(base_url.trim_end_matches('/'))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "API_BASE_URL cannot be a base URL")?;
        segments.extend(path_segments.iter().copied());
    }

    let response = client
        .get(url)
        .header("X-API-Key", api_key)
        .send()
        .await?
        .error_for_status()?;

    Ok(response.json::<Vec<LinkItem>>().await?)
}
