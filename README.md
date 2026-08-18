# PMC Explorer

Desktop app to search PubMed Central, open or save NCBI’s PDF, and optionally ask a local LM Studio model to translate a query or summarize JATS text.

This is not a PDF viewer and not a clinical decision tool. The paper is NCBI’s file. The in-app abstract/body dump is optional fuel for the local model.

## Requirements

- Rust (`rustup`)
- An NCBI developer email (sent as `tool=pmc_explorer` on E-utilities calls)
- LM Studio on `localhost:1234` only if you use natural-language search or cliff notes

## Run

```bash
git clone https://github.com/Divhanthelion/pubmed_desktop.git
cd pubmed_desktop
cargo run --release
```

Put your NCBI email in the field in the left panel. Searches are capped at 200 IDs.

LM Studio: start the local server on port 1234. Without it, keyword/author/journal search, Open PDF, and Save PDF still work.

## What it does

- **Query builder** — Keyword, author, journal → NCBI `esearch` (boolean `AND`).
- **Translate to PMC query** — Local LLM returns a boolean query string. That string is shown, then searched. It is not semantic search.
- **Open PDF / Save PDF** — System viewer, or download NCBI’s `/pdf/` file to a path you pick. HTML 200 bodies that are not `%PDF` are reported as no PDF.
- **Open HTML** — Article page in the system browser.
- **Cliff notes** — Sends parsed JATS abstract and body to the local model. Model text is not the paper.
- **Related links** — NCBI `elink`, first page of IDs.

Timeouts are 30s for NCBI/LLM HTTP and 120s for PDF download. Failures show as `NCBI:`, `LLM:`, `Parse:`, or `PDF:` instead of an empty spinner.

## Layout

- `src/main.rs` — eframe UI, Tokio tasks, error/query display, PDF actions.
- `src/pmc_api.rs` — Shared `reqwest` client, E-utilities (`esearch`, `esummary`, `efetch`, `elink`) with `tool`/`email`, idconv, local chat completions, JATS text extract, PDF fetch.
