mod pmc_api;

use directories::ProjectDirs;
use eframe::egui;
use pmc_api::{MAX_RETMAX, PmcQueryBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

#[derive(Serialize, Deserialize, Default)]
struct Preferences {
    ncbi_email: String,
}

fn preferences_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "pmc_explorer").map(|dirs| dirs.config_dir().join("prefs.json"))
}

fn load_preferences() -> Preferences {
    preferences_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_preferences(prefs: &Preferences) {
    if let Some(path) = preferences_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            path,
            serde_json::to_string_pretty(prefs).unwrap_or_default(),
        );
    }
}

struct PmcExplorerApp {
    keyword_input: String,
    author_input: String,
    journal_input: String,
    email_input: String,
    last_saved_email: String,

    natural_language_input: String,
    llm_summary: Arc<Mutex<Option<String>>>,
    llm_is_loading: Arc<Mutex<bool>>,

    results_per_page: usize,

    is_loading: Arc<Mutex<bool>>,
    search_results: Arc<Mutex<Option<Vec<String>>>>,
    last_error: Arc<Mutex<Option<String>>>,
    last_ncbi_query: Arc<Mutex<Option<String>>>,

    selected_pmcid: Option<String>,
    detail_loading: Arc<Mutex<bool>>,
    detail_title: Arc<Mutex<Option<String>>>,
    detail_ids: Arc<Mutex<Option<String>>>,
    detail_parsed_article: Arc<Mutex<Option<pmc_api::ParsedArticle>>>,
    detail_related_links: Arc<Mutex<Option<Vec<String>>>>,
    detail_url: Arc<Mutex<Option<String>>>,
    pdf_saving: Arc<Mutex<bool>>,
    last_notice: Arc<Mutex<Option<String>>>,

    rt: Runtime,
}

impl Default for PmcExplorerApp {
    fn default() -> Self {
        let prefs = load_preferences();
        Self {
            keyword_input: String::new(),
            author_input: String::new(),
            journal_input: String::new(),
            email_input: prefs.ncbi_email.clone(),
            last_saved_email: prefs.ncbi_email,
            natural_language_input: String::new(),
            llm_summary: Arc::new(Mutex::new(None)),
            llm_is_loading: Arc::new(Mutex::new(false)),
            results_per_page: 100,
            is_loading: Arc::new(Mutex::new(false)),
            search_results: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(None)),
            last_ncbi_query: Arc::new(Mutex::new(None)),
            selected_pmcid: None,
            detail_loading: Arc::new(Mutex::new(false)),
            detail_title: Arc::new(Mutex::new(None)),
            detail_ids: Arc::new(Mutex::new(None)),
            detail_parsed_article: Arc::new(Mutex::new(None)),
            detail_related_links: Arc::new(Mutex::new(None)),
            detail_url: Arc::new(Mutex::new(None)),
            pdf_saving: Arc::new(Mutex::new(false)),
            last_notice: Arc::new(Mutex::new(None)),
            rt: Runtime::new().expect("Failed to create Tokio runtime"),
        }
    }
}

fn set_opt(slot: &Arc<Mutex<Option<String>>>, value: Option<String>) {
    *slot.lock().unwrap() = value;
}

impl eframe::App for PmcExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("search_panel")
            .resizable(true)
            .min_size(250.0)
            .show_inside(ui, |ui| {
                ui.heading("AI Search Agent");
                ui.text_edit_multiline(&mut self.natural_language_input);

                ui.horizontal(|ui| {
                    ui.label("NCBI email:");
                    let response = ui.text_edit_singleline(&mut self.email_input);
                    if response.lost_focus() && self.email_input != self.last_saved_email {
                        self.last_saved_email = self.email_input.clone();
                        save_preferences(&Preferences {
                            ncbi_email: self.email_input.clone(),
                        });
                    }
                });
                ui.label(
                    egui::RichText::new("Developer email sent as NCBI tool=pmc_explorer.")
                        .italics()
                        .small(),
                );

                if ui.button("Translate to PMC Query & Search").clicked() {
                    *self.is_loading.lock().unwrap() = true;
                    *self.search_results.lock().unwrap() = None;
                    set_opt(&self.last_error, None);
                    set_opt(&self.last_ncbi_query, None);
                    let user_query = self.natural_language_input.clone();
                    let email = self.email_input.clone();
                    let results_clone = Arc::clone(&self.search_results);
                    let loading_clone = Arc::clone(&self.is_loading);
                    let error_clone = Arc::clone(&self.last_error);
                    let query_clone = Arc::clone(&self.last_ncbi_query);
                    let ctx_clone = ui.ctx().clone();
                    let results_per_page = self.results_per_page as u32;

                    self.rt.spawn(async move {
                        let system_prompt = "You are an NCBI E-utilities expert. Convert the user's natural language request into a highly sophisticated PMC boolean search query. Use appropriate tags like [Title/Abstract], [Author], or [Journal]. OUTPUT ONLY THE RAW QUERY STRING. Do not include any conversational text or markdown formatting.";

                        match pmc_api::ask_local_llm(system_prompt, &user_query).await {
                            Ok(ncbi_query) => {
                                let clean_query = ncbi_query.trim().trim_matches('"').to_string();
                                set_opt(&query_clone, Some(clean_query.clone()));
                                match pmc_api::search_pmc(&clean_query, results_per_page, &email)
                                    .await
                                {
                                    Ok(res) => {
                                        *results_clone.lock().unwrap() =
                                            Some(res.esearchresult.idlist);
                                    }
                                    Err(e) => {
                                        set_opt(&error_clone, Some(format!("NCBI: {e}")));
                                    }
                                }
                            }
                            Err(e) => {
                                set_opt(&error_clone, Some(format!("LLM: {e}")));
                            }
                        }
                        *loading_clone.lock().unwrap() = false;
                        ctx_clone.request_repaint();
                    });
                }

                ui.add_space(10.0);

                ui.heading("Query Builder");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Keyword:");
                    ui.text_edit_singleline(&mut self.keyword_input);
                });
                ui.horizontal(|ui| {
                    ui.label("Author:");
                    ui.text_edit_singleline(&mut self.author_input);
                });
                ui.horizontal(|ui| {
                    ui.label("Journal:");
                    ui.text_edit_singleline(&mut self.journal_input);
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label("Results per page:");
                    ui.add(
                        egui::DragValue::new(&mut self.results_per_page)
                            .speed(1.0)
                            .range(1..=MAX_RETMAX as usize),
                    );
                    ui.label(format!("(max {MAX_RETMAX})"));
                });
                ui.add_space(5.0);

                let loading = *self.is_loading.lock().unwrap();
                if loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Searching...");
                    });
                } else if ui.button("Search PMC").clicked() {
                    *self.is_loading.lock().unwrap() = true;
                    *self.search_results.lock().unwrap() = None;
                    set_opt(&self.last_error, None);

                    let mut builder = PmcQueryBuilder::new();
                    if !self.keyword_input.is_empty() {
                        builder = builder.add_keyword(&self.keyword_input);
                    }
                    if !self.author_input.is_empty() {
                        builder = builder.add_author(&self.author_input);
                    }
                    if !self.journal_input.is_empty() {
                        builder = builder.add_journal(&self.journal_input);
                    }

                    let query = builder.build();
                    set_opt(&self.last_ncbi_query, Some(query.clone()));
                    let email = self.email_input.clone();
                    let results_clone = Arc::clone(&self.search_results);
                    let loading_clone = Arc::clone(&self.is_loading);
                    let error_clone = Arc::clone(&self.last_error);
                    let ctx_clone = ui.ctx().clone();
                    let results_per_page = self.results_per_page as u32;

                    self.rt.spawn(async move {
                        match pmc_api::search_pmc(&query, results_per_page, &email).await {
                            Ok(res) => {
                                *results_clone.lock().unwrap() = Some(res.esearchresult.idlist);
                            }
                            Err(e) => {
                                set_opt(&error_clone, Some(format!("NCBI: {e}")));
                            }
                        }
                        *loading_clone.lock().unwrap() = false;
                        ctx_clone.request_repaint();
                    });
                }

                if let Some(query) = &*self.last_ncbi_query.lock().unwrap() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("NCBI query:").strong());
                    ui.label(query);
                }

                if let Some(err) = &*self.last_error.lock().unwrap() {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::from_rgb(180, 60, 60), err);
                }

                ui.separator();
                ui.heading("Results");

                let mut clicked_id = None;
                {
                    let results_lock = self.search_results.lock().unwrap();
                    if let Some(ids) = &*results_lock {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for id in ids {
                                if ui
                                    .selectable_label(
                                        self.selected_pmcid.as_deref() == Some(id),
                                        format!("PMCID: {}", id),
                                    )
                                    .clicked()
                                {
                                    clicked_id = Some(id.clone());
                                }
                            }
                        });
                    }
                }
                if let Some(id) = clicked_id {
                    self.load_article_details(id, ui.ctx().clone());
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Article Explorer");
            ui.separator();

            if self.selected_pmcid.is_none() {
                ui.label("Select a PMCID from the results to view details.");
                return;
            }

            self.pdf_actions(ui);

            if *self.detail_loading.lock().unwrap() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Fetching metadata, IDs, and Open Access full‑text...");
                });
                return;
            }

            if let Some(ids) = &*self.detail_ids.lock().unwrap() {
                ui.label(egui::RichText::new("Identifiers:").strong());
                ui.label(ids);
                ui.add_space(10.0);
            }

            if let Some(title) = &*self.detail_title.lock().unwrap() {
                ui.label(egui::RichText::new("Title:").strong());
                ui.label(title);
                ui.add_space(10.0);
            }

            ui.separator();
            if ui.button("🧠 Generate Cliff Notes (Local LLM)").clicked() {
                let text_to_summarize = self
                    .detail_parsed_article
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|p| format!("Abstract: {}\n\nBody: {}", p.abstract_text, p.body_text))
                    .unwrap_or_default();

                if text_to_summarize.trim().is_empty() {
                    set_opt(
                        &self.last_error,
                        Some("LLM: no article text to summarize".to_string()),
                    );
                } else {
                    *self.llm_is_loading.lock().unwrap() = true;
                    set_opt(&self.last_error, None);
                    let summary_arc = Arc::clone(&self.llm_summary);
                    let loading_arc = Arc::clone(&self.llm_is_loading);
                    let error_arc = Arc::clone(&self.last_error);
                    let ctx_clone = ui.ctx().clone();

                    self.rt.spawn(async move {
                        let system_prompt = "You are an expert medical researcher. Provide a highly structured, concise 'cliff notes' summary of the following study. Include: Key Objective, Methodology, Primary Findings, and Conclusion.";
                        match pmc_api::ask_local_llm(system_prompt, &text_to_summarize).await {
                            Ok(summary) => {
                                *summary_arc.lock().unwrap() = Some(summary);
                            }
                            Err(e) => {
                                set_opt(&error_arc, Some(format!("LLM: {e}")));
                            }
                        }
                        *loading_arc.lock().unwrap() = false;
                        ctx_clone.request_repaint();
                    });
                }
            }

            if *self.llm_is_loading.lock().unwrap() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("LLM is reading...");
                });
            } else if let Some(summary) = &*self.llm_summary.lock().unwrap() {
                ui.group(|ui| {
                    ui.heading("AI Summary");
                    ui.label(summary);
                });
            }
            ui.add_space(10.0);

            ui.label(egui::RichText::new("Open Access Full Text / Abstract:").strong());
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(parsed) = &*self.detail_parsed_article.lock().unwrap() {
                    if !parsed.abstract_text.is_empty() {
                        ui.heading("Abstract");
                        ui.label(&parsed.abstract_text);
                        ui.add_space(8.0);
                    }
                    if !parsed.body_text.is_empty() {
                        ui.heading("Body");
                        ui.label(&parsed.body_text);
                        ui.add_space(8.0);
                    }
                } else if self.detail_url.lock().unwrap().is_some() {
                    ui.label(
                        egui::RichText::new(
                            "Full text not available via XML. Use Open PDF or Open HTML.",
                        )
                        .italics(),
                    );
                } else {
                    ui.label(egui::RichText::new("Unable to retrieve article link.").italics());
                }

                ui.add_space(10.0);
                if let Some(links) = &*self.detail_related_links.lock().unwrap()
                    && !links.is_empty()
                {
                    ui.heading("Related / Cited Articles");
                    for link in links.iter().take(10) {
                        if ui.link(format!("PMCID: {}", link)).clicked() {
                            ui.ctx()
                                .open_url(egui::OpenUrl::same_tab(pmc_api::pmc_article_url(link)));
                        }
                    }
                }

            });
        });
    }
}

impl PmcExplorerApp {
    fn pdf_actions(&mut self, ui: &mut egui::Ui) {
        let Some(pmcid) = self.selected_pmcid.clone() else {
            return;
        };

        ui.horizontal(|ui| {
            if ui.button("Open PDF").clicked() {
                ui.ctx()
                    .open_url(egui::OpenUrl::same_tab(pmc_api::pmc_pdf_url(&pmcid)));
            }
            if ui.button("Open HTML").clicked() {
                ui.ctx()
                    .open_url(egui::OpenUrl::same_tab(pmc_api::pmc_article_url(&pmcid)));
            }

            let saving = *self.pdf_saving.lock().unwrap();
            if saving {
                ui.spinner();
                ui.label("Saving PDF...");
            } else if ui.button("Save PDF").clicked() {
                let default_name = format!("PMC{}.pdf", pmc_api::normalize_pmcid(&pmcid));
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name(&default_name)
                    .save_file()
                {
                    *self.pdf_saving.lock().unwrap() = true;
                    set_opt(&self.last_error, None);
                    set_opt(&self.last_notice, None);
                    let saving_arc = Arc::clone(&self.pdf_saving);
                    let error_arc = Arc::clone(&self.last_error);
                    let notice_arc = Arc::clone(&self.last_notice);
                    let ctx = ui.ctx().clone();
                    self.rt.spawn(async move {
                        match pmc_api::fetch_pmc_pdf(&pmcid).await {
                            Ok(bytes) => match std::fs::write(&path, bytes) {
                                Ok(()) => {
                                    set_opt(&notice_arc, Some(format!("Saved {}", path.display())));
                                }
                                Err(e) => {
                                    set_opt(&error_arc, Some(format!("PDF write: {e}")));
                                }
                            },
                            Err(e) => {
                                set_opt(&error_arc, Some(format!("PDF: {e}")));
                            }
                        }
                        *saving_arc.lock().unwrap() = false;
                        ctx.request_repaint();
                    });
                }
            }
        });

        if let Some(notice) = &*self.last_notice.lock().unwrap() {
            ui.label(notice);
        }
        if let Some(err) = &*self.last_error.lock().unwrap() {
            ui.colored_label(egui::Color32::from_rgb(180, 60, 60), err);
        }
        ui.add_space(8.0);
    }

    fn load_article_details(&mut self, pmcid: String, ctx: egui::Context) {
        self.selected_pmcid = Some(pmcid.clone());
        *self.detail_loading.lock().unwrap() = true;
        set_opt(&self.last_error, None);
        set_opt(&self.last_notice, None);

        *self.detail_title.lock().unwrap() = None;
        *self.detail_ids.lock().unwrap() = None;
        *self.detail_parsed_article.lock().unwrap() = None;
        *self.detail_related_links.lock().unwrap() = None;
        *self.detail_url.lock().unwrap() = None;
        *self.llm_summary.lock().unwrap() = None;
        *self.llm_is_loading.lock().unwrap() = false;

        let title_arc = Arc::clone(&self.detail_title);
        let ids_arc = Arc::clone(&self.detail_ids);
        let parsed_arc = Arc::clone(&self.detail_parsed_article);
        let links_arc = Arc::clone(&self.detail_related_links);
        let url_arc = Arc::clone(&self.detail_url);
        let loading_arc = Arc::clone(&self.detail_loading);
        let error_arc = Arc::clone(&self.last_error);
        let email = self.email_input.clone();

        self.rt.spawn(async move {
            let mut errors = Vec::new();

            match pmc_api::convert_ids(&pmcid, &email).await {
                Ok(conv_data) => {
                    if let Some(record) = conv_data.records.first() {
                        let doi = record.doi.as_deref().unwrap_or("N/A");
                        let pmid = record.pmid.as_deref().unwrap_or("N/A");
                        *ids_arc.lock().unwrap() = Some(format!("DOI: {} | PMID: {}", doi, pmid));
                    }
                }
                Err(e) => errors.push(format!("NCBI idconv: {e}")),
            }

            pmc_api::ncbi_rate_limit_pause().await;

            match pmc_api::fetch_pmc_summary(&pmcid, &email).await {
                Ok(summary) => {
                    if let Some(article_data) = summary.result.get(&pmcid)
                        && let Some(title) = article_data.get("title").and_then(|t| t.as_str())
                    {
                        *title_arc.lock().unwrap() = Some(title.to_string());
                    }
                }
                Err(e) => errors.push(format!("NCBI esummary: {e}")),
            }

            pmc_api::ncbi_rate_limit_pause().await;

            match pmc_api::fetch_pmc_fulltext_xml(&pmcid, &email).await {
                Ok(xml_data) => match pmc_api::parse_jats_xml(&xml_data) {
                    Ok(parsed) => {
                        if !parsed.abstract_text.is_empty() || !parsed.body_text.is_empty() {
                            *parsed_arc.lock().unwrap() = Some(parsed);
                        }
                    }
                    Err(e) => errors.push(format!("Parse: {e}")),
                },
                Err(e) => errors.push(format!("NCBI efetch: {e}")),
            }

            pmc_api::ncbi_rate_limit_pause().await;

            match pmc_api::fetch_pmc_links(&pmcid, &email).await {
                Ok(link_data) => {
                    if let Some(linksets) = link_data.linksets
                        && let Some(ls) = linksets.into_iter().next()
                        && let Some(dbs) = ls.linksetdbs
                        && let Some(db) = dbs.into_iter().next()
                    {
                        *links_arc.lock().unwrap() = Some(db.links);
                    }
                }
                Err(e) => errors.push(format!("NCBI elink: {e}")),
            }

            *url_arc.lock().unwrap() = Some(pmc_api::pmc_article_url(&pmcid));

            if !errors.is_empty() {
                set_opt(&error_arc, Some(errors.join("\n")));
            }

            *loading_arc.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "PMC Explorer - Native Apple Silicon",
        options,
        Box::new(|_cc| Ok(Box::new(PmcExplorerApp::default()))),
    )
}
