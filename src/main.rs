use eframe::egui;
use reqwest::{blocking::Client, Url};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
struct LinkItem {
    name: String,
    link: String,
}

struct GithubApiApp {
    api_base_url: String,
    api_key: String,
    username: String,
    repository: String,
    repositories: Vec<LinkItem>,
    contents: Vec<LinkItem>,
    status: String,
    client: Client,
}

impl Default for GithubApiApp {
    fn default() -> Self {
        Self {
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_owned()),
            api_key: env::var("API_KEY").unwrap_or_default(),
            username: env::var("GITHUB_USERNAME").unwrap_or_default(),
            repository: String::new(),
            repositories: Vec::new(),
            contents: Vec::new(),
            status: "Enter your API key and GitHub username to begin.".to_owned(),
            client: Client::new(),
        }
    }
}

impl eframe::App for GithubApiApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(context, |interface| {
            interface.heading("GitHub API App");
            interface.label("Read-only repository browser");
            interface.add_space(12.0);

            egui::Grid::new("connection_settings")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(interface, |fields| {
                    fields.label("API URL");
                    fields.text_edit_singleline(&mut self.api_base_url);
                    fields.end_row();

                    fields.label("API key");
                    fields.add(egui::TextEdit::singleline(&mut self.api_key).password(true));
                    fields.end_row();

                    fields.label("GitHub username");
                    fields.text_edit_singleline(&mut self.username);
                    fields.end_row();

                    fields.label("Repository");
                    fields.text_edit_singleline(&mut self.repository);
                    fields.end_row();
                });

            interface.add_space(12.0);
            interface.horizontal(|actions| {
                if actions.button("Load repositories").clicked() {
                    self.load_repositories();
                }
                if actions.button("Load contents").clicked() {
                    self.load_contents();
                }
                if actions.button("Clear").clicked() {
                    self.repositories.clear();
                    self.contents.clear();
                    self.status = "Results cleared.".to_owned();
                }
            });

            interface.separator();
            interface.label(&self.status);

            egui::SidePanel::left("repositories_panel")
                .resizable(true)
                .default_width(300.0)
                .show_inside(interface, |panel| {
                    panel.heading("Repositories");
                    egui::ScrollArea::vertical().show(panel, |list| {
                        for repository in &self.repositories {
                            list.hyperlink_to(&repository.name, &repository.link);
                        }
                    });
                });

            egui::CentralPanel::default().show_inside(interface, |panel| {
                panel.heading("Repository contents");
                egui::ScrollArea::vertical().show(panel, |list| {
                    for item in &self.contents {
                        list.hyperlink_to(&item.name, &item.link);
                    }
                });
            });
        });
    }
}

impl GithubApiApp {
    fn load_repositories(&mut self) {
        if self.validate_inputs().is_err() {
            return;
        }

        let username = self.username.clone();
        match self.get_links(&["v1", "users", &username, "repositories"]) {
            Ok(repositories) => {
                self.repositories = repositories;
                self.status = format!("Loaded {} repositories.", self.repositories.len());
            }
            Err(error) => self.status = format!("Repository request failed: {error}"),
        }
    }

    fn load_contents(&mut self) {
        if self.validate_inputs().is_err() {
            return;
        }
        if self.repository.trim().is_empty() {
            self.status = "Enter a repository name first.".to_owned();
            return;
        }

        let username = self.username.clone();
        let repository = self.repository.clone();
        match self.get_links(&["v1", "users", &username, "repos", &repository, "tree"]) {
            Ok(contents) => {
                self.contents = contents;
                self.status = format!("Loaded {} repository items.", self.contents.len());
            }
            Err(error) => self.status = format!("Contents request failed: {error}"),
        }
    }

    fn validate_inputs(&mut self) -> Result<(), ()> {
        if self.api_key.trim().is_empty() {
            self.status = "Enter an API key first.".to_owned();
            return Err(());
        }
        if self.username.trim().is_empty() {
            self.status = "Enter a GitHub username first.".to_owned();
            return Err(());
        }
        Ok(())
    }

    fn get_links(&self, path_segments: &[&str]) -> Result<Vec<LinkItem>, String> {
        let mut url = Url::parse(self.api_base_url.trim_end_matches('/'))
            .map_err(|error| format!("invalid API URL: {error}"))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| "API URL cannot be a base URL".to_owned())?;
            segments.extend(path_segments.iter().copied());
        }

        self.client
            .get(url)
            .header("X-API-Key", &self.api_key)
            .send()
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<Vec<LinkItem>>()
            .map_err(|error| error.to_string())
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 680.0])
            .with_min_inner_size([680.0, 460.0]),
        ..Default::default()
    };

    eframe::run_native(
        "GitHub API App",
        options,
        Box::new(|_creation_context| Ok(Box::new(GithubApiApp::default()))),
    )
}
