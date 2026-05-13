# PMC Explorer with Local AI

A blazing-fast, natively rendered desktop application designed to explore, harvest, and summarize open-access research articles from the **NCBI PubMed Central (PMC)** database using the power of Local LLMs.

Built entirely in Rust using the hardware-accelerated `eframe` / `egui` framework, PMC Explorer delivers a unified interface without relying on clunky embedded chromium instances. It natively parses JATS XML data from PMC and feeds it directly into your own private AI models to generate immediate summaries and execute highly-tuned semantic searches.

<img width="1070" height="940" alt="Screenshot 2026-05-12 at 6 44 42 PM" src="https://github.com/user-attachments/assets/5bf63691-7af0-447e-89be-b4b811f29255" />

---

## 🚀 The Ultimate Beginner's Setup Guide

This guide will take you from zero to running your own private AI-powered medical research assistant on your machine.

### Step 1: Install Rust
Rust is the incredibly fast programming language this application is built in.
1. Open your computer's terminal (Command Prompt/PowerShell on Windows, Terminal on macOS/Linux).
2. Paste the following command and hit Enter:
   - **Mac/Linux:** `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
   - **Windows:** Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs/).
3. Follow the on-screen prompts (choose option 1 for default installation).
4. Restart your terminal.

### Step 2: Install and Setup LM Studio
LM Studio allows you to run powerful AI models entirely offline for maximum privacy.
1. Download **LM Studio** from [lmstudio.ai](https://lmstudio.ai/) and install it.
2. Open LM Studio and use the search bar to find a model. 
   - *Recommendation:* If you have a powerful machine (16GB+ VRAM), search for **Qwen 2.5/3.6** or a high-context model. If you have a standard laptop, search for **Llama 3 8B** or **Mistral v0.3**.
   - Make sure you pick a model with a high "Context Window" (e.g., 32k or 65k) if you want the AI to read massive research papers.
3. Download the model (usually ends in `.gguf`).
4. On the left side of LM Studio, click the **Local Server** tab (the icon looks like a double-ended arrow `↔`).
5. Select the model you just downloaded from the top dropdown.
6. Ensure the port is set to `1234` (this is the default).
7. Click **Start Server**. Your local offline AI is now actively waiting for requests!

### Step 3: Run PMC Explorer
1. In your terminal, clone this repository (or download it as a ZIP and extract it):
   ```bash
   git clone <your-repository>
   cd PMC_Explorer
   ```
2. Run the application:
   ```bash
   cargo run --release
   ```
   *(Note: The first time you run this, Rust will take a minute or two to download and compile the interface libraries. Subsequent runs will be instant).*

---

## Features

- **AI Search Agent**: Type exactly what you are looking for in native English (e.g., *"How do mRNA vaccines affect myocardium in youth?"*). The Local LLM will instantly translate your request into a highly optimized, complex boolean PMC query string and fetch the results!
- **AI Cliff Notes Summarizer**: Click **"🧠 Generate Cliff Notes"** while viewing a massive 20,000+ word paper. The app will securely pipe the entire native JATS XML text straight to your offline LM Studio model and return a structured summary containing the *Objective*, *Methodology*, *Primary Findings*, and *Conclusion*.
- **Advanced Query Builder**: A clean left-hand side panel enabling manual filtering by **Keyword**, **Author**, and **Journal**.
- **Native JATS XML Parsing**: Uses NCBI `efetch` endpoints alongside `roxmltree` to parse raw JATS XML structures flawlessly in the background, rendering complex medical abstracts natively into the canvas.
- **Discovery Connections**: Fully unified with the `elink` API, automatically listing related studies and internal citations to easily bounce between related literature.

## Architecture & Code Structure
* **`src/main.rs`**: Core application bootstrapping, state management via Arc/Mutex, Tokio async runtime spawns, and all the reactive front-end `eframe` component declarations.
* **`src/pmc_api.rs`**: Handles complex underlying logic, including local OpenAI-compatible requests to `localhost:1234`, URL building for four major NCBI E-Utilities (`esearch`, `esummary`, `efetch`, and `elink`), and traversing raw XML DOM trees to cleanly format paragraphs.
