# PMC Explorer

A blazing-fast, natively rendered desktop application designed to explore, harvest, and read open-access research articles from the **NCBI PubMed Central (PMC)** database without leaving the environment. 

PMC Explorer is built entirely in Rust using the hardware-accelerated `eframe` / `egui` framework, delivering a unified and native interface without relying on clunky embedded chromium instances or external browser tabs.

![PMC Explorer UI](https://upload.wikimedia.org/wikipedia/commons/thumb/1/1b/Square_200x200.png/200px-Square_200x200.png) <!-- Feel free to replace this embedded screenshot placeholder with an actual screenshot! -->

## Features

- **Query Builder**: A clean left-hand side panel enabling quick filtering by **Keyword**, **Author**, and **Journal**, bridging seamlessly to custom advanced NCBI queries.
- **Dynamic Pagination**: Configure your strict "Results per page" parameters directly in UI (supporting the absolute esearch.fcgi maximum of 10,000 queries per pull).
- **Native JATS XML Parsing**: Completely ditches Webview architectures. Uses `efetch` endpoints alongside `roxmltree` to parse raw JATS XML structures flawlessly in the background. It surfaces `<abstract>` and `<body>` text formats immediately natively into the `egui` canvas for immediate reading.
- **Discovery Connections**: Fully unified with `elink` API, automatically listing related studies and internal citations (`pmc_pmc_cites`) at the bottom of articles to easily bounce between related literature.
- **Concurrency Driven**: Architected on `tokio`, meaning complex API fetches (ID resolution, ESummary, EFetch XML parsing, ELink retrieval) happen seamlessly and completely asynchronously without locking up your frame rate.

## Installation

You will need the standard Rust toolchain installed:

```bash
git clone <your-repository>
cd PMC_Explorer
cargo run --release
```

## Structure

* **`src/main.rs`**: Core application bootstrapping, state management (Arc Mutex sharing strings across standard Tokio worker threads), and front-end `eframe` declarations.
* **`src/pmc_api.rs`**: Handles complex underlying URL serialization across four major NCBI E-Utilities (`esearch`, `esummary`, `efetch`, and `elink`), as well as executing the deep XML Tree manipulation to normalize paragraphs into Rust strings.

## Limitations

- Some medical databases on NCBI do not offer complete structures via their open-access E-Utilities portal. Articles that do not expose an `<abstract>` or `<body>` element will gracefully notify you that the XML cannot be seamlessly fetched, relying on the fallback System Browser link
