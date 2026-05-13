mod pmc_api;

use eframe::egui;
use pmc_api::PmcQueryBuilder;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

struct PmcExplorerApp {
    keyword_input: String,
    author_input: String,
    journal_input: String,
    
    natural_language_input: String,
    llm_summary: Arc<Mutex<Option<String>>>,
    llm_is_loading: Arc<Mutex<bool>>,

    /// How many results to ask NCBI for per search.
    /// Default 100 (you can raise it up to 10 000 if you really want a huge list).
    results_per_page: usize,

    is_loading: Arc<Mutex<bool>>,
    search_results: Arc<Mutex<Option<Vec<String>>>>,

    selected_pmcid: Option<String>,
    detail_loading: Arc<Mutex<bool>>,
    detail_title: Arc<Mutex<Option<String>>>,
    detail_ids: Arc<Mutex<Option<String>>>,
    detail_parsed_article: Arc<Mutex<Option<pmc_api::ParsedArticle>>>,
    detail_related_links: Arc<Mutex<Option<Vec<String>>>>,
    detail_url: Arc<Mutex<Option<String>>>, // <-- link to the article on PMC

    rt: Runtime,
}

impl Default for PmcExplorerApp {
    fn default() -> Self {
        Self {
            keyword_input: String::new(),
            author_input: String::new(),
            journal_input: String::new(),
            natural_language_input: String::new(),
            llm_summary: Arc::new(Mutex::new(None)),
            llm_is_loading: Arc::new(Mutex::new(false)),
            results_per_page: 100, // <-- sensible default
            is_loading: Arc::new(Mutex::new(false)),
            search_results: Arc::new(Mutex::new(None)),
            selected_pmcid: None,
            detail_loading: Arc::new(Mutex::new(false)),
            detail_title: Arc::new(Mutex::new(None)),
            detail_ids: Arc::new(Mutex::new(None)),
            detail_parsed_article: Arc::new(Mutex::new(None)),
            detail_related_links: Arc::new(Mutex::new(None)),
            detail_url: Arc::new(Mutex::new(None)),
            rt: Runtime::new().expect("Failed to create Tokio runtime"),
        }
    }
}

impl eframe::App for PmcExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        
        // --- LEFT PANEL: Search and Results ---
        egui::Panel::left("search_panel")
            .resizable(true)
            .min_size(250.0)
            .show_inside(ui, |ui| {
                ui.heading("AI Search Agent");
                ui.text_edit_multiline(&mut self.natural_language_input);

                if ui.button("Translate to PMC Query & Search").clicked() {
                    *self.is_loading.lock().unwrap() = true;
                    let user_query = self.natural_language_input.clone();
                    let results_clone = Arc::clone(&self.search_results);
                    let loading_clone = Arc::clone(&self.is_loading);
                    let ctx_clone = ui.ctx().clone();
                    let results_per_page = self.results_per_page as u32;

                    self.rt.spawn(async move {
                        let system_prompt = "You are an NCBI E-utilities expert. Convert the user's natural language request into a highly sophisticated PMC boolean search query. Use appropriate tags like [Title/Abstract], [Author], or [Journal]. OUTPUT ONLY THE RAW QUERY STRING. Do not include any conversational text or markdown formatting.";
                        
                        if let Ok(ncbi_query) = pmc_api::ask_local_llm(system_prompt, &user_query).await {
                            // Trim any accidental whitespace or quotes the LLM might add
                            let clean_query = ncbi_query.trim().trim_matches('"').to_string();
                            
                            // Now pass this strictly formatted query directly to your existing search API
                            if let Ok(res) = pmc_api::search_pmc(&clean_query, results_per_page).await {
                                *results_clone.lock().unwrap() = Some(res.esearchresult.idlist);
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

                // ----- NEW: Results‑per‑page selector -----
                ui.horizontal(|ui| {
                    ui.label("Results per page:");
                    ui.add(
                        egui::DragValue::new(&mut self.results_per_page)
                            .speed(1.0)
                            .range(1..=10_000),
                    );
                    ui.label("(max 10 000)");
                });
                ui.add_space(5.0);

                let loading = *self.is_loading.lock().unwrap();
                if loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Searching...");
                    });
                } else {
                    if ui.button("Search PMC").clicked() {
                        *self.is_loading.lock().unwrap() = true;

                        let mut builder = PmcQueryBuilder::new();
                        if !self.keyword_input.is_empty() { builder = builder.add_keyword(&self.keyword_input); }
                        if !self.author_input.is_empty() { builder = builder.add_author(&self.author_input); }
                        if !self.journal_input.is_empty() { builder = builder.add_journal(&self.journal_input); }

                        // Optional: keep the Open Access filter if you want to guarantee full‑text availability
                        // if self.only_open_access { builder = builder.add_keyword("open access[filter]"); }

                        let query = builder.build();
                        let results_clone = Arc::clone(&self.search_results);
                        let loading_clone = Arc::clone(&self.is_loading);
                        let ctx_clone = ui.ctx().clone();
                        let results_per_page = self.results_per_page as u32;

                        // NOTE: we now pass the user‑chosen `results_per_page`
                        self.rt.spawn(async move {
                            if let Ok(res) = pmc_api::search_pmc(
                                &query,
                                results_per_page,
                            ).await {
                                *results_clone.lock().unwrap() = Some(res.esearchresult.idlist);
                            }
                            *loading_clone.lock().unwrap() = false;
                            ctx_clone.request_repaint();
                        });
                    }
                }

                ui.separator();
                ui.heading("Results");

                // Separate click detection from mutable method call to avoid borrow‑checker issues
                let mut clicked_id = None;
                {
                    let results_lock = self.search_results.lock().unwrap();
                    if let Some(ids) = &*results_lock {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for id in ids {
                                if ui.selectable_label(self.selected_pmcid.as_deref() == Some(id), format!("PMCID: {}", id)).clicked() {
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

        // --- CENTRAL PANEL: Article Details ---
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Article Explorer");
            ui.separator();

            if *self.detail_loading.lock().unwrap() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Fetching metadata, IDs, and Open Access full‑text...");
                });
                return;
            }

            if self.selected_pmcid.is_none() {
                ui.label("Select a PMCID from the results to view details.");
                return;
            }

            // Identifiers (DOI | PMID)
            if let Some(ids) = &*self.detail_ids.lock().unwrap() {
                ui.label(egui::RichText::new("Identifiers:").strong());
                ui.label(ids);
                ui.add_space(10.0);
            }

            // Title
            if let Some(title) = &*self.detail_title.lock().unwrap() {
                ui.label(egui::RichText::new("Title:").strong());
                ui.label(title);
                ui.add_space(10.0);
            }

            ui.separator();
            if ui.button("🧠 Generate Cliff Notes (Local LLM)").clicked() {
                *self.llm_is_loading.lock().unwrap() = true;
                let text_to_summarize = self.detail_parsed_article.lock().unwrap()
                    .as_ref()
                    .map(|p| format!("Abstract: {}\n\nBody: {}", p.abstract_text, p.body_text))
                    .unwrap_or_default();

                let summary_arc = Arc::clone(&self.llm_summary);
                let loading_arc = Arc::clone(&self.llm_is_loading);
                let ctx_clone = ui.ctx().clone();

                self.rt.spawn(async move {
                    let system_prompt = "You are an expert medical researcher. Provide a highly structured, concise 'cliff notes' summary of the following study. Include: Key Objective, Methodology, Primary Findings, and Conclusion.";
                    if let Ok(summary) = pmc_api::ask_local_llm(system_prompt, &text_to_summarize).await {
                        *summary_arc.lock().unwrap() = Some(summary);
                    }
                    *loading_arc.lock().unwrap() = false;
                    ctx_clone.request_repaint();
                });
            }

            // Display the summary if it exists
            if *self.llm_is_loading.lock().unwrap() {
                ui.horizontal(|ui| { ui.spinner(); ui.label("LLM is reading..."); });
            } else if let Some(summary) = &*self.llm_summary.lock().unwrap() {
                ui.group(|ui| {
                    ui.heading("AI Summary");
                    ui.label(summary);
                });
            }
            ui.add_space(10.0);

            // Fully Native Open Access Text
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
                } else {
                    // No OA full text – show a clickable link to the article on PMC
                    if let Some(_url) = &*self.detail_url.lock().unwrap() {
                        ui.label(egui::RichText::new("Full text not seamlessly available via XML. Fallback option:").italics());
                    } else {
                        ui.label(egui::RichText::new("Unable to retrieve article link.").italics());
                    }
                }

                ui.add_space(10.0);
                if let Some(links) = &*self.detail_related_links.lock().unwrap() {
                    if !links.is_empty() {
                        ui.heading("Related / Cited Articles");
                        for link in links.iter().take(10) {
                            if ui.link(format!("PMCID: {}", link)).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::same_tab(format!("https://www.ncbi.nlm.nih.gov/pmc/articles/PMC{}/", link)));
                            }
                        }
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                if let Some(url) = &*self.detail_url.lock().unwrap() {
                    if ui.button("↗ System Browser").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::same_tab(url.clone()));
                    }
                }
            });
        });
    }
}

impl PmcExplorerApp {
    /// Dispatches concurrent async tasks to fetch all article data at once
    fn load_article_details(&mut self, pmcid: String, ctx: egui::Context) {
        self.selected_pmcid = Some(pmcid.clone());
        *self.detail_loading.lock().unwrap() = true;
        
        // Reset previous data
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

        self.rt.spawn(async move {
            // Task 1: ID Conversion (DOI | PMID)
            if let Ok(conv_data) = pmc_api::convert_ids(&pmcid).await {
                if let Some(record) = conv_data.records.first() {
                    let doi = record.doi.as_deref().unwrap_or("N/A");
                    let pmid = record.pmid.as_deref().unwrap_or("N/A");
                    *ids_arc.lock().unwrap() = Some(format!("DOI: {} | PMID: {}", doi, pmid));
                }
            }

            // Task 2: Metadata (Title) via ESummary
            if let Ok(summary) = pmc_api::fetch_pmc_summary(&pmcid).await {
                if let Some(article_data) = summary.result.get(&pmcid) {
                    if let Some(title) = article_data.get("title").and_then(|t| t.as_str()) {
                        *title_arc.lock().unwrap() = Some(title.to_string());
                    }
                }
            }

            // Task 3: Full Text parsing via Efetch JATS XML
            if let Ok(xml_data) = pmc_api::fetch_pmc_fulltext_xml(&pmcid).await {
                let parsed = pmc_api::parse_jats_xml(&xml_data);
                if !parsed.abstract_text.is_empty() || !parsed.body_text.is_empty() {
                    *parsed_arc.lock().unwrap() = Some(parsed);
                }
            }

            // Task 4: ELink related tracking
            if let Ok(link_data) = pmc_api::fetch_pmc_links(&pmcid).await {
                if let Some(linksets) = link_data.linksets {
                    if let Some(ls) = linksets.into_iter().next() {
                        if let Some(dbs) = ls.linksetdbs {
                            if let Some(db) = dbs.into_iter().next() {
                                *links_arc.lock().unwrap() = Some(db.links);
                            }
                        }
                    }
                }
            }

            // Always construct the PMC article URL as a fallback
            *url_arc.lock().unwrap() = Some(format!("https://www.ncbi.nlm.nih.gov/pmc/articles/PMC{}/", pmcid));

            *loading_arc.lock().unwrap() = false;
            ctx.request_repaint(); // Wake UI thread to show fetched details
        });
    }
}

// Initialization for eframe v0.34
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
