use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use url::form_urlencoded;

// --- DATA MODELS ---
#[derive(Deserialize, Debug)]
pub struct ESearchResult {
    pub esearchresult: ESearchData,
}

#[derive(Deserialize, Debug)]
pub struct ESearchData {
    pub count: String,
    pub idlist: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct IdConvResponse {
    pub records: Vec<IdRecord>,
}

#[derive(Deserialize, Debug)]
pub struct IdRecord {
    pub pmcid: Option<String>,
    pub pmid: Option<String>,
    pub doi: Option<String>,
}


#[derive(Deserialize, Debug)]
pub struct ESummaryResponse {
    pub result: HashMap<String, serde_json::Value>, 
}

#[derive(Deserialize, Debug)]
pub struct ELinkResponse {
    pub linksets: Option<Vec<ELinkSet>>,
}

#[derive(Deserialize, Debug)]
pub struct ELinkSet {
    pub dbfrom: Option<String>,
    pub linksetdbs: Option<Vec<ELinkSetDb>>,
}

#[derive(Deserialize, Debug)]
pub struct ELinkSetDb {
    pub dbto: String,
    pub linkname: String,
    pub links: Vec<String>,
}

pub struct ParsedArticle {
    pub abstract_text: String,
    pub body_text: String,
}

// --- QUERY BUILDER ---
pub struct PmcQueryBuilder {
    pub terms: Vec<String>,
}

impl PmcQueryBuilder {
    pub fn new() -> Self {
        Self { terms: Vec::new() }
    }
    pub fn add_keyword(mut self, keyword: &str) -> Self {
        self.terms.push(keyword.to_string());
        self
    }
    pub fn add_author(mut self, author: &str) -> Self {
        self.terms.push(format!("{}[Author]", author));
        self
    }
    pub fn add_journal(mut self, journal: &str) -> Self {
        self.terms.push(format!("{}[Journal]", journal));
        self
    }
    pub fn build(&self) -> String {
        self.terms.join(" AND ")
    }
}

// --- API ENDPOINTS ---
pub async fn search_pmc(
    query: &str,
    retmax: u32,
) -> Result<ESearchResult, reqwest::Error> {
    let client = Client::new();
    let encoded_query: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("term", query)
        .append_pair("retmax", &retmax.to_string())
        .finish();
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pmc&retmode=json&{}", encoded_query);
    client.get(&url).send().await?.json().await
}

pub async fn convert_ids(ids: &str) -> Result<IdConvResponse, reqwest::Error> {
    let client = Client::new();
    let url = format!("https://www.ncbi.nlm.nih.gov/pmc/utils/idconv/v1.0/?ids={}&format=json", ids);
    client.get(&url).send().await?.json().await
}

pub async fn fetch_pmc_summary(pmcid: &str) -> Result<ESummaryResponse, reqwest::Error> {
    let client = Client::new();
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pmc&id={}&retmode=json", pmcid);
    client.get(&url).send().await?.json().await
}

const LM_STUDIO_URL: &str = "http://localhost:1234/v1/chat/completions";

pub async fn ask_local_llm(system_prompt: &str, user_prompt: &str) -> Result<String, reqwest::Error> {
    let client = Client::new();
    
    let payload = json!({
        "model": "local-model", 
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.3 
    });

    let res = client.post(LM_STUDIO_URL)
        .json(&payload)
        .send()
        .await?;

    let json_res: serde_json::Value = res.json().await?;
    
    // Extract the text response
    let content = json_res["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Error parsing LLM response")
        .to_string();

    Ok(content)
}


pub async fn fetch_pmc_links(pmcid: &str) -> Result<ELinkResponse, reqwest::Error> {
    let client = Client::new();
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/elink.fcgi?dbfrom=pmc&db=pmc&id={}&retmode=json", pmcid);
    client.get(&url).send().await?.json().await
}

pub async fn fetch_pmc_fulltext_xml(pmcid: &str) -> Result<String, reqwest::Error> {
    let client = Client::new();
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pmc&id={}&retmode=xml", pmcid);
    client.get(&url).send().await?.text().await
}

fn extract_text(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    for desc in node.descendants() {
        if desc.is_text() {
            if let Some(t) = desc.text() {
                text.push_str(t);
            }
        }
    }
    text
}

pub fn parse_jats_xml(xml: &str) -> ParsedArticle {
    let mut abstract_text = String::new();
    let mut body_text = String::new();

    let opt = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    if let Ok(doc) = roxmltree::Document::parse_with_options(xml, opt) {
        // Find abstract
        if let Some(abstract_node) = doc.descendants().find(|n| n.has_tag_name("abstract")) {
            for p in abstract_node.descendants().filter(|n| n.has_tag_name("p") || n.has_tag_name("title")) {
                let text = extract_text(&p).trim().to_string();
                if !text.is_empty() {
                    abstract_text.push_str(&text);
                    abstract_text.push_str("\n\n");
                }
            }
        }
        
        // Find body
        if let Some(body_node) = doc.descendants().find(|n| n.has_tag_name("body")) {
            for p in body_node.descendants().filter(|n| n.has_tag_name("p") || n.has_tag_name("title")) {
                let text = extract_text(&p).trim().to_string();
                if !text.is_empty() {
                    body_text.push_str(&text);
                    body_text.push_str("\n\n");
                }
            }
        }
    }

    ParsedArticle {
        abstract_text: abstract_text.trim().to_string(),
        body_text: body_text.trim().to_string(),
    }
}
