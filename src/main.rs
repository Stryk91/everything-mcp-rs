//! Everything MCP Server in Rust - with CLI mode

use clap::{Parser, Subcommand};
use rmcp::{
    ServerHandler, ServiceExt,
    model::{ServerInfo, ServerCapabilities, Implementation, ProtocolVersion, CallToolResult, Content},
    tool, tool_router, tool_handler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    transport::stdio,
    ErrorData as McpError,
};
use libloading::{Library, Symbol};
use schemars::JsonSchema;
use serde::Deserialize;
use widestring::U16CString;
use once_cell::sync::Lazy;
use std::sync::Mutex;

type SetSearchFn = unsafe extern "system" fn(*const u16);
type SetU32Fn = unsafe extern "system" fn(u32);
type SetI32Fn = unsafe extern "system" fn(i32);
type QueryFn = unsafe extern "system" fn(i32) -> i32;
type GetU32Fn = unsafe extern "system" fn() -> u32;
type GetPathFn = unsafe extern "system" fn(u32, *mut u16, u32);
type GetAttrFn = unsafe extern "system" fn(u32) -> u32;
type IsLoadedFn = unsafe extern "system" fn() -> i32;

struct EvDll {
    set_search: Symbol<'static, SetSearchFn>,
    set_max: Symbol<'static, SetU32Fn>,
    set_case: Symbol<'static, SetI32Fn>,
    set_word: Symbol<'static, SetI32Fn>,
    set_regex: Symbol<'static, SetI32Fn>,
    set_path: Symbol<'static, SetI32Fn>,
    set_flags: Symbol<'static, SetU32Fn>,
    query: Symbol<'static, QueryFn>,
    get_num: Symbol<'static, GetU32Fn>,
    get_tot: Symbol<'static, GetU32Fn>,
    get_path: Symbol<'static, GetPathFn>,
    get_attr: Symbol<'static, GetAttrFn>,
    get_err: Symbol<'static, GetU32Fn>,
    is_loaded: Symbol<'static, IsLoadedFn>,
    get_ver: [Symbol<'static, GetU32Fn>; 4],
}

impl EvDll {
    fn load() -> Result<Self, String> {
        unsafe {
            let lib = Library::new("Everything64.dll")
                .or_else(|_| Library::new("C:\\Program Files\\Everything\\Everything64.dll"))
                .map_err(|e| e.to_string())?;
            let lib: &'static Library = Box::leak(Box::new(lib));
            
            Ok(Self {
                set_search: lib.get(b"Everything_SetSearchW\0").map_err(|e| e.to_string())?,
                set_max: lib.get(b"Everything_SetMax\0").map_err(|e| e.to_string())?,
                set_case: lib.get(b"Everything_SetMatchCase\0").map_err(|e| e.to_string())?,
                set_word: lib.get(b"Everything_SetMatchWholeWord\0").map_err(|e| e.to_string())?,
                set_regex: lib.get(b"Everything_SetRegex\0").map_err(|e| e.to_string())?,
                set_path: lib.get(b"Everything_SetMatchPath\0").map_err(|e| e.to_string())?,
                set_flags: lib.get(b"Everything_SetRequestFlags\0").map_err(|e| e.to_string())?,
                query: lib.get(b"Everything_QueryW\0").map_err(|e| e.to_string())?,
                get_num: lib.get(b"Everything_GetNumResults\0").map_err(|e| e.to_string())?,
                get_tot: lib.get(b"Everything_GetTotResults\0").map_err(|e| e.to_string())?,
                get_path: lib.get(b"Everything_GetResultFullPathNameW\0").map_err(|e| e.to_string())?,
                get_attr: lib.get(b"Everything_GetResultAttributes\0").map_err(|e| e.to_string())?,
                get_err: lib.get(b"Everything_GetLastError\0").map_err(|e| e.to_string())?,
                is_loaded: lib.get(b"Everything_IsDBLoaded\0").map_err(|e| e.to_string())?,
                get_ver: [
                    lib.get(b"Everything_GetMajorVersion\0").map_err(|e| e.to_string())?,
                    lib.get(b"Everything_GetMinorVersion\0").map_err(|e| e.to_string())?,
                    lib.get(b"Everything_GetRevision\0").map_err(|e| e.to_string())?,
                    lib.get(b"Everything_GetBuildNumber\0").map_err(|e| e.to_string())?,
                ],
            })
        }
    }
}

static DLL: Lazy<Mutex<Option<EvDll>>> = Lazy::new(|| Mutex::new(EvDll::load().ok()));

fn search(q: &str, max: u32, case: bool, word: bool, regex: bool, path: bool) -> String {
    let guard = match DLL.lock() { Ok(g) => g, Err(e) => return format!("Lock: {}", e) };
    let dll = match guard.as_ref() { Some(d) => d, None => return "DLL not loaded".into() };
    
    unsafe {
        let qw = match U16CString::from_str(q) { Ok(s) => s, Err(e) => return format!("Query: {}", e) };
        (dll.set_search)(qw.as_ptr());
        (dll.set_max)(max.clamp(1, 500));
        (dll.set_case)(case as i32);
        (dll.set_word)(word as i32);
        (dll.set_regex)(regex as i32);
        (dll.set_path)(path as i32);
        (dll.set_flags)(0x113);
        
        if (dll.query)(1) == 0 { return format!("Query failed ({}). Is Everything running?", (dll.get_err)()); }
        
        let n = (dll.get_num)();
        if n == 0 { return format!("No results for: {}", q); }
        
        let mut out = format!("Found {} (showing {}):\n\n", (dll.get_tot)(), n);
        let mut buf = vec![0u16; 32768];
        
        for i in 0..n {
            (dll.get_path)(i, buf.as_mut_ptr(), buf.len() as u32);
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let s = String::from_utf16_lossy(&buf[..end]);
            let dir = ((dll.get_attr)(i) & 0x10) != 0;
            out.push_str(&format!("{} {}\n", if dir { "[DIR]" } else { "[FILE]" }, s));
        }
        out
    }
}

// Parameter structs with Parameters wrapper pattern
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchReq { 
    #[schemars(description = "Search query")] pub query: String,
    pub max_results: Option<u32>, pub match_case: Option<bool>, 
    pub whole_word: Option<bool>, pub regex: Option<bool>, pub match_path: Option<bool>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExtReq { #[schemars(description = "Extensions")] pub extensions: String, pub keywords: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyReq { pub keywords: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FolderReq { pub folder_path: String, pub query: String, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecentReq { pub days: Option<u32>, pub extension: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DateReq { pub date_filter: String, pub keywords: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SizeReq { pub size_filter: String, pub keywords: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LargeReq { pub min_size: Option<String>, pub file_type: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContentReq { pub content: String, pub extensions: Option<String>, pub folder: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegexReq { pub pattern: String, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DupeReq { pub pattern: String, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExcludeReq { pub query: String, pub exclude: String, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrReq { pub terms: String, pub and_filter: Option<String>, pub max_results: Option<u32> }
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FoldersReq { pub query: String, pub max_results: Option<u32> }

// Server implementation
#[derive(Clone)]
pub struct EvMcp { tool_router: ToolRouter<Self> }

#[tool_router]
impl EvMcp {
    pub fn new() -> Self { Self { tool_router: Self::tool_router() } }

    #[tool(description = "Search files/folders. Supports wildcards, ext:, paths, regex.")]
    async fn everything_search(&self, Parameters(p): Parameters<SearchReq>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(search(&p.query, p.max_results.unwrap_or(50), p.match_case.unwrap_or(false), p.whole_word.unwrap_or(false), p.regex.unwrap_or(false), p.match_path.unwrap_or(false)))]))
    }

    #[tool(description = "Check Everything status")]
    async fn everything_status(&self) -> Result<CallToolResult, McpError> {
        let r = match DLL.lock() {
            Ok(g) => match g.as_ref() {
                Some(dll) => unsafe {
                    if (dll.is_loaded)() != 0 {
                        format!("v{}.{}.{}.{} Ready", (dll.get_ver[0])(), (dll.get_ver[1])(), (dll.get_ver[2])(), (dll.get_ver[3])())
                    } else { "Not available".into() }
                },
                None => "DLL not loaded".into(),
            },
            Err(e) => format!("Error: {}", e),
        };
        Ok(CallToolResult::success(vec![Content::text(r)]))
    }

    #[tool(description = "Search by extension(s)")]
    async fn everything_search_ext(&self, Parameters(p): Parameters<ExtReq>) -> Result<CallToolResult, McpError> {
        let eq: String = p.extensions.split(',').map(|e| format!("ext:{}", e.trim().trim_start_matches('.'))).collect::<Vec<_>>().join(" | ");
        let q = p.keywords.filter(|k| !k.is_empty()).map(|k| format!("({}) {}", eq, k)).unwrap_or(eq);
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search audio files")]
    async fn everything_search_audio(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:mp3;wav;flac;aac;ogg;wma;m4a".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search video files")]
    async fn everything_search_video(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:mp4;avi;mkv;mov;wmv;flv;webm".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search image files")]
    async fn everything_search_image(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:jpg;jpeg;png;gif;bmp;svg;webp;ico".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search documents")]
    async fn everything_search_doc(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:pdf;doc;docx;xls;xlsx;ppt;pptx;txt;md".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search code files")]
    async fn everything_search_code(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:cs;py;js;ts;java;cpp;c;h;go;rs;rb;php;ps1".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search archives")]
    async fn everything_search_archive(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:zip;rar;7z;tar;gz;bz2;iso".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search executables")]
    async fn everything_search_exe(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "ext:exe;msi;bat;cmd;ps1;sh".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search in folder")]
    async fn everything_search_in_folder(&self, Parameters(p): Parameters<FolderReq>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(search(&format!("\"{}\\\" {}", p.folder_path, p.query), p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search folders only")]
    async fn everything_search_folders(&self, Parameters(p): Parameters<FoldersReq>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(search(&format!("folder: {}", p.query), p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Recently modified files")]
    async fn everything_recent(&self, Parameters(p): Parameters<RecentReq>) -> Result<CallToolResult, McpError> {
        let mut q = format!("dm:last{}days", p.days.unwrap_or(1));
        if let Some(ext) = p.extension.filter(|e| !e.is_empty()) { q.push_str(&format!(" ext:{}", ext.trim_start_matches('.'))); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search by date created")]
    async fn everything_search_date_created(&self, Parameters(p): Parameters<DateReq>) -> Result<CallToolResult, McpError> {
        let mut q = format!("dc:{}", p.date_filter);
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search by date modified")]
    async fn everything_search_date_modified(&self, Parameters(p): Parameters<DateReq>) -> Result<CallToolResult, McpError> {
        let mut q = format!("dm:{}", p.date_filter);
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search by size")]
    async fn everything_search_size(&self, Parameters(p): Parameters<SizeReq>) -> Result<CallToolResult, McpError> {
        let mut q = format!("size:{}", p.size_filter);
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Find large files")]
    async fn everything_search_large(&self, Parameters(p): Parameters<LargeReq>) -> Result<CallToolResult, McpError> {
        let mut q = format!("size:>{}", p.min_size.as_deref().unwrap_or("100mb"));
        if let Some(ft) = p.file_type {
            q.push_str(match ft.to_lowercase().as_str() {
                "video" => " ext:mp4;avi;mkv;mov",
                "audio" => " ext:mp3;wav;flac",
                "archive" => " ext:zip;rar;7z;iso",
                _ => ""
            });
        }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Find empty folders")]
    async fn everything_search_empty(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let q = p.keywords.filter(|k| !k.is_empty()).map(|k| format!("empty: {}", k)).unwrap_or("empty:".into());
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search hidden files")]
    async fn everything_search_hidden(&self, Parameters(p): Parameters<KeyReq>) -> Result<CallToolResult, McpError> {
        let mut q = "attrib:H".to_string();
        if let Some(k) = p.keywords.filter(|k| !k.is_empty()) { q.push_str(&format!(" {}", k)); }
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search file contents (SLOW)")]
    async fn everything_search_content(&self, Parameters(p): Parameters<ContentReq>) -> Result<CallToolResult, McpError> {
        let mut q = String::new();
        if let Some(f) = p.folder.filter(|f| !f.is_empty()) { q.push_str(&format!("\"{}\\\" ", f)); }
        if let Some(e) = p.extensions.filter(|e| !e.is_empty()) { q.push_str(&format!("ext:{} ", e.replace(',', ";"))); }
        q.push_str(&format!("content:\"{}\"", p.content));
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(20), false, false, false, false))]))
    }

    #[tool(description = "Search with regex")]
    async fn everything_search_regex(&self, Parameters(p): Parameters<RegexReq>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(search(&p.pattern, p.max_results.unwrap_or(50), false, false, true, false))]))
    }

    #[tool(description = "Find duplicates by name")]
    async fn everything_find_duplicates(&self, Parameters(p): Parameters<DupeReq>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(search(&format!("dupe: {}", p.pattern), p.max_results.unwrap_or(100), false, false, false, false))]))
    }

    #[tool(description = "Search with exclusions")]
    async fn everything_search_exclude(&self, Parameters(p): Parameters<ExcludeReq>) -> Result<CallToolResult, McpError> {
        let ex: Vec<String> = p.exclude.split(',').map(|s| format!("!{}", s.trim())).collect();
        Ok(CallToolResult::success(vec![Content::text(search(&format!("{} {}", p.query, ex.join(" ")), p.max_results.unwrap_or(50), false, false, false, false))]))
    }

    #[tool(description = "Search with OR logic")]
    async fn everything_search_or(&self, Parameters(p): Parameters<OrReq>) -> Result<CallToolResult, McpError> {
        let oq = p.terms.split(',').map(|s| s.trim()).collect::<Vec<_>>().join(" | ");
        let q = p.and_filter.filter(|f| !f.is_empty()).map(|f| format!("({}) {}", oq, f)).unwrap_or(oq);
        Ok(CallToolResult::success(vec![Content::text(search(&q, p.max_results.unwrap_or(50), false, false, false, false))]))
    }
}

#[tool_handler]
impl ServerHandler for EvMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Everything Search MCP (Rust) - 24 tools".into()),
        }
    }
}

// CLI mode
#[derive(Parser)]
#[command(name = "everything")]
#[command(about = "Everything Search - CLI + MCP modes")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search files/folders
    Search {
        /// Search query
        query: String,
        /// Max results
        #[arg(short = 'n', long, default_value = "20")]
        max: u32,
        /// Match case
        #[arg(short = 'c', long)]
        case: bool,
        /// Use regex
        #[arg(short = 'r', long)]
        regex: bool,
    },
    /// Search by extension
    Ext {
        /// Extensions (comma-separated)
        extensions: String,
        /// Keywords
        #[arg(short = 'k', long)]
        keywords: Option<String>,
        #[arg(short = 'n', long, default_value = "20")]
        max: u32,
    },
    /// Recently modified files
    Recent {
        /// Days back
        #[arg(short = 'd', long, default_value = "1")]
        days: u32,
        /// Extension filter
        #[arg(short = 'e', long)]
        ext: Option<String>,
        #[arg(short = 'n', long, default_value = "20")]
        max: u32,
    },
    /// Large files
    Large {
        /// Min size (e.g. 100mb)
        #[arg(short = 's', long, default_value = "100mb")]
        size: String,
        #[arg(short = 'n', long, default_value = "20")]
        max: u32,
    },
    /// Check Everything status
    Status,
    /// Run as MCP server (default if no args)
    Mcp,
}

fn cli_status() {
    match DLL.lock() {
        Ok(g) => match g.as_ref() {
            Some(dll) => unsafe {
                if (dll.is_loaded)() != 0 {
                    println!("Everything v{}.{}.{}.{} - Ready",
                        (dll.get_ver[0])(), (dll.get_ver[1])(), (dll.get_ver[2])(), (dll.get_ver[3])());
                } else {
                    eprintln!("Everything not available. Is it running?");
                    std::process::exit(1);
                }
            },
            None => {
                eprintln!("Everything64.dll not loaded");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Search { query, max, case, regex }) => {
            println!("{}", search(&query, max, case, false, regex, false));
        }
        Some(Commands::Ext { extensions, keywords, max }) => {
            let eq: String = extensions.split(',').map(|e| format!("ext:{}", e.trim().trim_start_matches('.'))).collect::<Vec<_>>().join(" | ");
            let q = keywords.filter(|k| !k.is_empty()).map(|k| format!("({}) {}", eq, k)).unwrap_or(eq);
            println!("{}", search(&q, max, false, false, false, false));
        }
        Some(Commands::Recent { days, ext, max }) => {
            let mut q = format!("dm:last{}days", days);
            if let Some(e) = ext.filter(|e| !e.is_empty()) { q.push_str(&format!(" ext:{}", e.trim_start_matches('.'))); }
            println!("{}", search(&q, max, false, false, false, false));
        }
        Some(Commands::Large { size, max }) => {
            println!("{}", search(&format!("size:>{}", size), max, false, false, false, false));
        }
        Some(Commands::Status) => {
            cli_status();
        }
        Some(Commands::Mcp) | None => {
            // MCP server mode
            let server = EvMcp::new().serve(stdio()).await?;
            server.waiting().await?;
        }
    }
    Ok(())
}


