use eframe::egui;
use egui::text::{LayoutJob, TextFormat};
use reqwest::{blocking::Client, Url};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

#[derive(Debug, Deserialize, Clone)]
struct LinkItem {
    name: String,
    link: String,
}

enum LoadResult {
    Repositories(Result<Vec<LinkItem>, String>),
    Contents(Result<Vec<LinkItem>, String>),
}

struct GithubApiApp {
    api_base_url: String,
    api_key: String,
    username: String,
    organization: bool,
    repository: String,
    repositories: Vec<LinkItem>,
    contents: Vec<LinkItem>,
    status: String,
    download_dir: PathBuf,
    ide_open: bool,
    ide_files: Vec<PathBuf>,
    open_file: Option<PathBuf>,
    editor_text: String,
    request_receiver: Option<Receiver<LoadResult>>,
    loading: bool,
    client: Client,
}

impl Default for GithubApiApp {
    fn default() -> Self {
        Self {
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_owned()),
            api_key: env::var("API_KEY").unwrap_or_default(),
            username: env::var("GITHUB_USERNAME").unwrap_or_default(),
            organization: false,
            repository: String::new(),
            repositories: Vec::new(),
            contents: Vec::new(),
            status: "Enter your API key and GitHub username to begin.".to_owned(),
            download_dir: env::var("DOWNLOAD_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("downloads")),
            ide_open: false,
            ide_files: Vec::new(),
            open_file: None,
            editor_text: String::new(),
            request_receiver: None,
            loading: false,
            client: Client::new(),
        }
    }
}

impl eframe::App for GithubApiApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_request();
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

                    fields.label("Owner is organization");
                    fields.checkbox(&mut self.organization, "Use /orgs endpoints");
                    fields.end_row();

                    fields.label("Repository");
                    fields.text_edit_singleline(&mut self.repository);
                    fields.end_row();
                });

            interface.add_space(12.0);
            interface.horizontal(|actions| {
                if actions
                    .add_enabled(!self.loading, egui::Button::new("Load repositories"))
                    .clicked()
                {
                    self.load_repositories();
                }
                if actions
                    .add_enabled(!self.loading, egui::Button::new("Load contents"))
                    .clicked()
                {
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
                        for repository in self.repositories.clone() {
                            let selected = self.repository == repository.name
                                || self.repository
                                    == repository.name.rsplit('/').next().unwrap_or("");
                            if list.selectable_label(selected, &repository.name).clicked() {
                                self.repository = repository
                                    .name
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or(&repository.name)
                                    .to_owned();
                                self.contents.clear();
                                self.status = format!("Selected {}.", self.repository);
                                self.load_contents();
                            }
                        }
                    });
                });

            egui::CentralPanel::default().show_inside(interface, |panel| {
                panel.heading("Repository contents");
                egui::ScrollArea::vertical().show(panel, |list| {
                    for item in self.contents.clone() {
                        if list.button(format!("Download {}", item.name)).clicked() {
                            self.download_item(&item);
                        }
                    }
                });
            });
        });

        self.show_ide(context);
    }
}

impl GithubApiApp {
    fn load_repositories(&mut self) {
        if self.validate_inputs().is_err() {
            return;
        }

        let username = self.username.clone();
        let owner_kind = if self.organization { "orgs" } else { "users" };
        let base_url = self.api_base_url.clone();
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        let (sender, receiver) = mpsc::channel();
        self.request_receiver = Some(receiver);
        self.loading = true;
        self.status = "Loading repositories...".to_owned();
        thread::spawn(move || {
            let result = request_links(
                &client,
                &api_key,
                &base_url,
                &["v1", owner_kind, &username, "repositories"],
            );
            let _ = sender.send(LoadResult::Repositories(result));
        });
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
        let owner_kind = if self.organization { "orgs" } else { "users" };
        let base_url = self.api_base_url.clone();
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        let (sender, receiver) = mpsc::channel();
        self.request_receiver = Some(receiver);
        self.loading = true;
        self.status = "Loading repository contents...".to_owned();
        thread::spawn(move || {
            let result = request_links(
                &client,
                &api_key,
                &base_url,
                &["v1", owner_kind, &username, "repos", &repository, "tree"],
            );
            let _ = sender.send(LoadResult::Contents(result));
        });
    }

    fn poll_request(&mut self) {
        let result = self
            .request_receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(result) = result else {
            return;
        };

        self.request_receiver = None;
        self.loading = false;
        match result {
            LoadResult::Repositories(Ok(repositories)) => {
                self.repositories = repositories;
                self.status = format!("Loaded {} repositories.", self.repositories.len());
            }
            LoadResult::Repositories(Err(error)) => {
                self.status = format!("Repository request failed: {error}");
            }
            LoadResult::Contents(Ok(contents)) => {
                self.contents = contents;
                self.status = format!("Loaded {} repository items.", self.contents.len());
            }
            LoadResult::Contents(Err(error)) => {
                self.status = format!("Contents request failed: {error}");
            }
        }
    }

    fn download_item(&mut self, item: &LinkItem) {
        if !item.link.contains("/blob/") {
            self.status = format!("{} is a folder; download a file inside it.", item.name);
            return;
        }

        let owner_kind = if self.organization { "orgs" } else { "users" };
        let owner = self.username.clone();
        let repository = self.repository.clone();
        let path = item.name.clone();
        match self.get_file(
            &["v1", owner_kind, &owner, "repos", &repository, "file"],
            &path,
        ) {
            Ok(bytes) => {
                let relative_path = safe_relative_path(&path);
                let local_path = self
                    .download_dir
                    .join(&owner)
                    .join(&repository)
                    .join(relative_path);
                match local_path
                    .parent()
                    .ok_or_else(|| "invalid local file path".to_owned())
                    .and_then(|parent| {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())
                    })
                    .and_then(|_| fs::write(&local_path, bytes).map_err(|error| error.to_string()))
                {
                    Ok(()) => {
                        self.open_local_file(local_path);
                        self.status = format!("Saved {} locally.", path);
                    }
                    Err(error) => self.status = format!("Could not save file: {error}"),
                }
            }
            Err(error) => self.status = format!("Download failed: {error}"),
        }
    }

    fn open_local_file(&mut self, path: PathBuf) {
        self.editor_text = fs::read_to_string(&path)
            .unwrap_or_else(|_| "Binary or non-text file. It was saved locally.".to_owned());
        self.open_file = Some(path);
        self.refresh_ide_files();
        self.ide_open = true;
    }

    fn refresh_ide_files(&mut self) {
        self.ide_files.clear();
        let root = self
            .download_dir
            .join(self.username.trim())
            .join(self.repository.trim());
        collect_files(&root, &mut self.ide_files);
        self.ide_files.sort();
    }

    fn show_ide(&mut self, context: &egui::Context) {
        if !self.ide_open {
            return;
        }

        let viewport = egui::ViewportId::from_hash_of("github-api-ide");
        context.show_viewport_immediate(
            viewport,
            egui::ViewportBuilder::default()
                .with_title("GitHub API IDE")
                .with_inner_size([1000.0, 700.0]),
            |context, _class| {
                if context.input(|input| input.viewport().close_requested()) {
                    self.ide_open = false;
                    return;
                }
                egui::TopBottomPanel::top("ide_toolbar").show(context, |toolbar| {
                    toolbar.horizontal(|actions| {
                        actions.heading("Local IDE");
                        if actions.button("Save locally").clicked() {
                            if let Some(path) = &self.open_file {
                                match fs::write(path, &self.editor_text) {
                                    Ok(()) => self.status = "Saved local changes.".to_owned(),
                                    Err(error) => self.status = format!("Save failed: {error}"),
                                }
                            }
                        }
                        if actions.button("Close window").clicked() {
                            self.ide_open = false;
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });

                egui::SidePanel::left("ide_files")
                    .resizable(true)
                    .default_width(280.0)
                    .show(context, |panel| {
                        panel.heading("Downloaded files");
                        egui::ScrollArea::vertical().show(panel, |list| {
                            for path in self.ide_files.clone() {
                                let selected = self.open_file.as_ref() == Some(&path);
                                let label = path
                                    .strip_prefix(&self.download_dir)
                                    .unwrap_or(&path)
                                    .display()
                                    .to_string();
                                if list.selectable_label(selected, label).clicked() {
                                    self.open_local_file(path.clone());
                                }
                            }
                        });
                    });

                egui::CentralPanel::default().show(context, |editor| {
                    if let Some(path) = &self.open_file {
                        editor.label(path.display().to_string());
                    }
                    let file_path = self.open_file.clone();
                    let mut layouter = move |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                        let job = highlight_code(text, file_path.as_deref());
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    editor.add(
                        egui::TextEdit::multiline(&mut self.editor_text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(30)
                            .layouter(&mut layouter),
                    );
                });
            },
        );
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

    fn get_file(&self, path_segments: &[&str], path: &str) -> Result<Vec<u8>, String> {
        let mut url = Url::parse(self.api_base_url.trim_end_matches('/'))
            .map_err(|error| format!("invalid API URL: {error}"))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| "API URL cannot be a base URL".to_owned())?;
            segments.extend(path_segments.iter().copied());
        }
        url.query_pairs_mut().append_pair("path", path);

        self.client
            .get(url)
            .header("X-API-Key", &self.api_key)
            .send()
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| error.to_string())
    }
}

fn request_links(
    client: &Client,
    api_key: &str,
    base_url: &str,
    path_segments: &[&str],
) -> Result<Vec<LinkItem>, String> {
    let mut url = Url::parse(base_url.trim_end_matches('/'))
        .map_err(|error| format!("invalid API URL: {error}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "API URL cannot be a base URL".to_owned())?;
        segments.extend(path_segments.iter().copied());
    }

    client
        .get(url)
        .header("X-API-Key", api_key)
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json::<Vec<LinkItem>>()
        .map_err(|error| error.to_string())
}

fn safe_relative_path(path: &str) -> PathBuf {
    path.split('/')
        .filter(|component| !component.is_empty() && *component != "." && *component != "..")
        .fold(PathBuf::new(), |mut output, component| {
            output.push(component);
            output
        })
}

fn highlight_code(text: &str, path: Option<&Path>) -> LayoutJob {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let syntax = path
        .and_then(|path| syntax_set.find_syntax_for_file(path).ok().flatten())
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| theme_set.themes.values().next())
        .expect("syntect includes a default theme");
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut job = LayoutJob::default();

    for line in LinesWithEndings::from(text) {
        let ranges = highlighter
            .highlight_line(line, &syntax_set)
            .unwrap_or_default();
        for (style, highlighted) in ranges {
            job.append(
                highlighted,
                0.0,
                TextFormat {
                    color: egui::Color32::from_rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    ),
                    font_id: egui::FontId::monospace(14.0),
                    ..Default::default()
                },
            );
        }
    }
    job
}

fn collect_files(root: &PathBuf, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
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
