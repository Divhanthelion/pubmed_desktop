use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use url::form_urlencoded;

pub const MAX_RETMAX: u32 = 200;
const NCBI_TOOL: &str = "pmc_explorer";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const PDF_TIMEOUT: Duration = Duration::from_secs(120);
const LM_STUDIO_URL: &str = "http://localhost:1234/v1/chat/completions";

#[derive(Debug)]
pub enum PmcError {
    Network(reqwest::Error),
    LlmMissingContent,
    NcbiIdentity,
    Parse(String),
    NoPdf,
}

impl std::fmt::Display for PmcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => {
                if e.is_timeout() {
                    write!(f, "timed out")
                } else if let Some(status) = e.status() {
                    write!(f, "HTTP {status}")
                } else {
                    write!(f, "{e}")
                }
            }
            Self::LlmMissingContent => write!(f, "LLM response had no content"),
            Self::NcbiIdentity => write!(f, "NCBI developer email is required"),
            Self::Parse(msg) => write!(f, "{msg}"),
            Self::NoPdf => write!(f, "no PDF for this article"),
        }
    }
}

impl From<reqwest::Error> for PmcError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e)
    }
}

fn http_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .expect("reqwest client")
    })
}

fn ncbi_email(email: &str) -> Result<&str, PmcError> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        Err(PmcError::NcbiIdentity)
    } else {
        Ok(email)
    }
}

fn eutils_query(pairs: &[(&str, &str)], email: &str) -> Result<String, PmcError> {
    let email = ncbi_email(email)?;
    let mut ser = form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        ser.append_pair(k, v);
    }
    ser.append_pair("tool", NCBI_TOOL);
    ser.append_pair("email", email);
    Ok(ser.finish())
}

async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, PmcError> {
    let res = http_client().get(url).send().await?.error_for_status()?;
    Ok(res.json().await?)
}

async fn get_text(url: &str) -> Result<String, PmcError> {
    let res = http_client().get(url).send().await?.error_for_status()?;
    Ok(res.text().await?)
}

pub fn normalize_pmcid(pmcid: &str) -> &str {
    let pmcid = pmcid.trim();
    pmcid
        .strip_prefix("PMC")
        .or_else(|| pmcid.strip_prefix("pmc"))
        .unwrap_or(pmcid)
}

pub fn pmc_article_url(pmcid: &str) -> String {
    format!(
        "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC{}/",
        normalize_pmcid(pmcid)
    )
}

pub fn pmc_pdf_url(pmcid: &str) -> String {
    format!(
        "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC{}/pdf/",
        normalize_pmcid(pmcid)
    )
}

fn looks_like_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF")
}

pub async fn fetch_pmc_pdf(pmcid: &str) -> Result<Vec<u8>, PmcError> {
    let res = http_client()
        .get(pmc_pdf_url(pmcid))
        .timeout(PDF_TIMEOUT)
        .send()
        .await?
        .error_for_status()?;
    let bytes = res.bytes().await?;
    if !looks_like_pdf(&bytes) {
        return Err(PmcError::NoPdf);
    }
    Ok(bytes.to_vec())
}

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
    email: &str,
) -> Result<ESearchResult, PmcError> {
    let retmax = retmax.clamp(1, MAX_RETMAX);
    let qs = eutils_query(
        &[
            ("db", "pmc"),
            ("retmode", "json"),
            ("term", query),
            ("retmax", &retmax.to_string()),
        ],
        email,
    )?;
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?{qs}");
    get_json(&url).await
}

pub async fn convert_ids(ids: &str) -> Result<IdConvResponse, PmcError> {
    let qs = form_urlencoded::Serializer::new(String::new())
        .append_pair("ids", ids)
        .append_pair("format", "json")
        .finish();
    let url = format!("https://www.ncbi.nlm.nih.gov/pmc/utils/idconv/v1.0/?{qs}");
    get_json(&url).await
}

pub async fn fetch_pmc_summary(pmcid: &str, email: &str) -> Result<ESummaryResponse, PmcError> {
    let qs = eutils_query(
        &[("db", "pmc"), ("id", pmcid), ("retmode", "json")],
        email,
    )?;
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?{qs}");
    get_json(&url).await
}

pub async fn ask_local_llm(system_prompt: &str, user_prompt: &str) -> Result<String, PmcError> {
    let payload = json!({
        "model": "local-model",
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.3
    });

    let res = http_client()
        .post(LM_STUDIO_URL)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    let json_res: serde_json::Value = res.json().await?;
    llm_content_from_response(&json_res)
}

fn llm_content_from_response(json_res: &serde_json::Value) -> Result<String, PmcError> {
    match json_res["choices"][0]["message"]["content"].as_str() {
        Some(content) if !content.trim().is_empty() => Ok(content.to_string()),
        _ => Err(PmcError::LlmMissingContent),
    }
}

pub async fn fetch_pmc_links(pmcid: &str, email: &str) -> Result<ELinkResponse, PmcError> {
    let qs = eutils_query(
        &[
            ("dbfrom", "pmc"),
            ("db", "pmc"),
            ("id", pmcid),
            ("retmode", "json"),
        ],
        email,
    )?;
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/elink.fcgi?{qs}");
    get_json(&url).await
}

pub async fn fetch_pmc_fulltext_xml(pmcid: &str, email: &str) -> Result<String, PmcError> {
    let qs = eutils_query(&[("db", "pmc"), ("id", pmcid), ("retmode", "xml")], email)?;
    let url = format!("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?{qs}");
    get_text(&url).await
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

pub fn parse_jats_xml(xml: &str) -> Result<ParsedArticle, PmcError> {
    let mut abstract_text = String::new();
    let mut body_text = String::new();

    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(xml, opt)
        .map_err(|e| PmcError::Parse(e.to_string()))?;

    if let Some(abstract_node) = doc.descendants().find(|n| n.has_tag_name("abstract")) {
        for p in abstract_node
            .descendants()
            .filter(|n| n.has_tag_name("p") || n.has_tag_name("title"))
        {
            let text = extract_text(&p).trim().to_string();
            if !text.is_empty() {
                abstract_text.push_str(&text);
                abstract_text.push_str("\n\n");
            }
        }
    }

    if let Some(body_node) = doc.descendants().find(|n| n.has_tag_name("body")) {
        for p in body_node
            .descendants()
            .filter(|n| n.has_tag_name("p") || n.has_tag_name("title"))
        {
            let text = extract_text(&p).trim().to_string();
            if !text.is_empty() {
                body_text.push_str(&text);
                body_text.push_str("\n\n");
            }
        }
    }

    Ok(ParsedArticle {
        abstract_text: abstract_text.trim().to_string(),
        body_text: body_text.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn llm_missing_content_is_err() {
        let empty = json!({ "choices": [{ "message": {} }] });
        assert!(matches!(
            llm_content_from_response(&empty),
            Err(PmcError::LlmMissingContent)
        ));
    }

    #[test]
    fn llm_error_string_is_not_a_query() {
        let fallback = json!({
            "choices": [{ "message": { "content": "" } }]
        });
        assert!(matches!(
            llm_content_from_response(&fallback),
            Err(PmcError::LlmMissingContent)
        ));
    }

    #[test]
    fn pmcid_prefix_is_not_doubled() {
        assert_eq!(normalize_pmcid("PMC123"), "123");
        assert_eq!(pmc_pdf_url("PMC123"), pmc_pdf_url("123"));
        assert_eq!(
            pmc_pdf_url("123"),
            "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC123/pdf/"
        );
    }

    #[test]
    fn html_body_is_not_a_pdf() {
        assert!(!looks_like_pdf(b"<html>not a pdf</html>"));
        assert!(looks_like_pdf(b"%PDF-1.4"));
    }
}
