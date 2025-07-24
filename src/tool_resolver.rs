//! # Tool Path Resolver for Electron
//! 
//! This module handles finding optimization tools in different environments:
//! - Bundled with Electron app
//! - System-installed tools
//! - Development environment

use std::path::{Path, PathBuf};
use std::env;
use tracing::{debug, warn};

/// Tool path resolver for different deployment environments
pub struct ToolPathResolver {
    /// Base directory where tools are bundled (for Electron)
    tools_dir: Option<PathBuf>,
    /// Whether we're running in development mode
    is_development: bool,
}

impl ToolPathResolver {
    /// Create a new path resolver
    pub fn new() -> Self {
        let is_development = cfg!(debug_assertions) || env::var("NODE_ENV").map(|v| v == "development").unwrap_or(false);
        
        // Always try to detect tools directory, even in development
        let tools_dir = Self::detect_bundled_tools_dir();

        Self {
            tools_dir,
            is_development,
        }
    }

    /// Detect the bundled tools directory in Electron
    fn detect_bundled_tools_dir() -> Option<PathBuf> {
        let current_dir = env::current_dir().ok()?;
        debug!("Current directory: {:?}", current_dir);
        
        // Strategy 1: Check environment variable set by Electron (our custom setup)
        if let Ok(resources_path) = env::var("ELECTRON_RESOURCES_PATH") {
            let tools_path = PathBuf::from(resources_path).join("tools");
            debug!("Checking Electron resources path: {:?}", tools_path);
            if tools_path.exists() {
                debug!("Found tools directory via ELECTRON_RESOURCES_PATH: {:?}", tools_path);
                return Some(tools_path);
            }
        }
        
        // Strategy 2: Check TOOLS_DIR environment variable (direct override)
        if let Ok(tools_dir) = env::var("TOOLS_DIR") {
            let tools_path = PathBuf::from(tools_dir);
            debug!("Checking TOOLS_DIR environment variable: {:?}", tools_path);
            if tools_path.exists() {
                debug!("Found tools directory via TOOLS_DIR: {:?}", tools_path);
                return Some(tools_path);
            }
        }
        
        // Strategy 3: Look for tools directory by traversing up the directory tree
        let mut search_dir = current_dir.clone();
        for _ in 0..10 { // Max 10 levels up
            let tools_path = search_dir.join("src").join("tools");
            debug!("Checking tools path: {:?}", tools_path);
            if tools_path.exists() {
                debug!("Found tools directory by traversing up: {:?}", tools_path);
                return Some(tools_path);
            }
            
            // Go up one level
            if let Some(parent) = search_dir.parent() {
                search_dir = parent.to_path_buf();
            } else {
                break;
            }
        }
        
        // Strategy 4: Try to detect Electron app directory structure for production
        if let Ok(exe_path) = env::current_exe() {
            debug!("Executable path: {:?}", exe_path);
            if let Some(app_dir) = exe_path.parent() {
                debug!("App directory: {:?}", app_dir);
                
                // Common Electron app structures
                let possible_paths = [
                    app_dir.join("resources").join("tools"),               // Most common for Electron
                    app_dir.join("resources").join("app").join("tools"),   // Alternative structure
                    app_dir.join("tools"),                                 // Direct in app dir
                    app_dir.join("resources").join("app").join("src").join("tools"), // Development-like
                ];

                for path in &possible_paths {
                    debug!("Checking production path: {:?}", path);
                    if path.exists() {
                        debug!("Found bundled tools directory: {:?}", path);
                        return Some(path.clone());
                    }
                }
            }
        }

        warn!("No bundled tools directory found");
        None
    }

    /// Resolve the path to a specific tool
    pub fn resolve_tool(&self, tool_name: &str) -> Option<PathBuf> {
        debug!("Resolving tool: {}", tool_name);
        debug!("Tools directory: {:?}", self.tools_dir);
        
        // On Linux, prefer system tools
        if cfg!(target_os = "linux") {
            if let Some(system_path) = self.find_in_system_path(tool_name) {
                debug!("Using system tool on Linux: {} -> {:?}", tool_name, system_path);
                return Some(system_path);
            }
        }
        
        // On Windows/macOS, try bundled tools first
        if !cfg!(target_os = "linux") {
            if let Some(ref tools_dir) = self.tools_dir {
                let bundled_path = self.get_bundled_tool_path(tools_dir, tool_name);
                debug!("Checking bundled path: {:?}", bundled_path);
                if bundled_path.exists() {
                    debug!("Using bundled tool: {} -> {:?}", tool_name, bundled_path);
                    return Some(bundled_path);
                } else {
                    debug!("Bundled path does not exist: {:?}", bundled_path);
                }
            } else {
                debug!("No tools directory configured");
            }

            // Fall back to system PATH for Windows/macOS
            if let Some(system_path) = self.find_in_system_path(tool_name) {
                debug!("Using system tool as fallback: {} -> {:?}", tool_name, system_path);
                return Some(system_path);
            }
        }

        warn!("Tool not found: {}", tool_name);
        None
    }

    /// Get the expected path for a bundled tool
    fn get_bundled_tool_path(&self, tools_dir: &Path, tool_name: &str) -> PathBuf {
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            env::consts::OS
        };
        
        let extension = if cfg!(target_os = "windows") { ".exe" } else { "" };
        
        // Try different possible structures:
        // 1. Direct in platform folder: tools/{platform}/{tool_name}.exe
        let direct_path = tools_dir
            .join(platform)
            .join(format!("{}{}", tool_name, extension));
        
        if direct_path.exists() {
            return direct_path;
        }
        
        // 2. In tool-specific subfolder: tools/{platform}/{tool_name}/{tool_name}.exe
        let subfolder_path = tools_dir
            .join(platform)
            .join(tool_name)
            .join(format!("{}{}", tool_name, extension));
        
        if subfolder_path.exists() {
            return subfolder_path;
        }
        
        // Default to direct path if neither exists (for error reporting)
        direct_path
    }

    /// Find tool in system PATH
    fn find_in_system_path(&self, tool_name: &str) -> Option<PathBuf> {
        let extension = if cfg!(windows) { ".exe" } else { "" };
        let tool_with_ext = format!("{}{}", tool_name, extension);

        env::var_os("PATH")?
            .to_str()?
            .split(if cfg!(windows) { ';' } else { ':' })
            .map(|dir| Path::new(dir).join(&tool_with_ext))
            .find(|path| path.exists())
    }

    /// Check if a specific tool is available
    pub fn is_tool_available(&self, tool_name: &str) -> bool {
        self.resolve_tool(tool_name).is_some()
    }

    /// Get all available tools
    pub fn get_available_tools(&self) -> Vec<String> {
        let all_tools = [
            "cwebp", "dwebp",
            "mozjpeg", "jpegoptim", "jpegtran",
            "oxipng", "optipng", "pngcrush",
            "ffmpeg", "ffprobe",
            "exiftool"
        ];

        all_tools
            .iter()
            .filter(|&&tool| self.is_tool_available(tool))
            .map(|&tool| tool.to_string())
            .collect()
    }

    /// Helper methods for specific tools (convenience wrappers)
    
    /// Get path to cwebp tool
    pub fn cwebp(&self) -> Option<PathBuf> {
        self.resolve_tool("cwebp")
    }

    /// Get path to cjpeg tool (mozjpeg)
    pub fn cjpeg(&self) -> Option<PathBuf> {
        self.resolve_tool("cjpeg")
    }

    /// Get path to djpeg tool (mozjpeg)
    pub fn djpeg(&self) -> Option<PathBuf> {
        self.resolve_tool("djpeg")
    }

    /// Get path to oxipng tool
    pub fn oxipng(&self) -> Option<PathBuf> {
        self.resolve_tool("oxipng")
    }

    /// Get path to ffmpeg tool
    pub fn ffmpeg(&self) -> Option<PathBuf> {
        self.resolve_tool("ffmpeg")
    }

    /// Get path to ffprobe tool
    pub fn ffprobe(&self) -> Option<PathBuf> {
        self.resolve_tool("ffprobe")
    }

    /// Check if all required tools are available
    pub fn verify_tools(&self) -> Result<(), String> {
        let required_tools = ["cwebp", "cjpeg", "djpeg", "oxipng", "ffmpeg"];
        let mut missing_messages = Vec::new();

        for tool in &required_tools {
            if let Err(msg) = self.check_tool_with_instructions(tool) {
                missing_messages.push(msg);
            }
        }

        if missing_messages.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Some required tools are missing:\n\n{}",
                missing_messages.join("\n\n")
            ))
        }
    }

    /// Get a report of tool availability
    pub fn get_tools_report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!("Tool Path Resolver Report\n"));
        report.push_str(&format!("Development mode: {}\n", self.is_development));
        if cfg!(target_os = "linux") {
            report.push_str("Platform: Linux (using system tools)\n");
        } else {
            report.push_str(&format!("Bundled tools dir: {:?}\n", self.tools_dir));
        }
        report.push_str("\nTool Availability:\n");

        let tools = [
            ("WebP", vec!["cwebp", "dwebp"]),
            ("JPEG", vec!["cjpeg", "djpeg", "mozjpeg", "jpegoptim", "jpegtran"]),
            ("PNG", vec!["oxipng", "optipng", "pngcrush"]),
            ("Video", vec!["ffmpeg", "ffprobe"]),
            ("Metadata", vec!["exiftool"]),
        ];

        for (category, tool_list) in tools {
            report.push_str(&format!("\n{}:\n", category));
            for tool in tool_list {
                match self.check_tool_with_instructions(tool) {
                    Ok(path) => {
                        report.push_str(&format!("  ✅ {} -> {:?}\n", tool, path));
                    }
                    Err(msg) => {
                        if cfg!(target_os = "linux") {
                            let install_cmd = self.get_linux_install_instructions(tool);
                            report.push_str(&format!("  ❌ {} (install with: {})\n", tool, install_cmd));
                        } else {
                            report.push_str(&format!("  ❌ {} (not found)\n", tool));
                        }
                    }
                }
            }
        }

        if cfg!(target_os = "linux") {
            report.push_str("\nNote: On Linux, this tool uses system-installed binaries.\n");
            report.push_str("Install missing tools using the commands shown above.\n");
        }

        report
    }

    /// Get installation instructions for a tool on Linux
    fn get_linux_install_instructions(&self, tool_name: &str) -> String {
        match tool_name {
            "cwebp" | "dwebp" => "sudo apt-get install webp".to_string(),
            "cjpeg" | "djpeg" | "jpegtran" => "sudo apt-get install libjpeg-progs".to_string(),
            "mozjpeg" => "sudo apt-get install libjpeg-progs  # (provides cjpeg, djpeg, jpegtran)".to_string(),
            "oxipng" => "sudo apt-get install oxipng  # or download from: https://github.com/shssoichiro/oxipng/releases".to_string(),
            "optipng" => "sudo apt-get install optipng".to_string(),
            "pngcrush" => "sudo apt-get install pngcrush".to_string(),
            "ffmpeg" | "ffprobe" => "sudo apt-get install ffmpeg".to_string(),
            "exiftool" => "sudo apt-get install libimage-exiftool-perl".to_string(),
            "jpegoptim" => "sudo apt-get install jpegoptim".to_string(),
            _ => format!("sudo apt-get install {}", tool_name),
        }
    }

    /// Check if a tool is available and provide installation instructions if not
    pub fn check_tool_with_instructions(&self, tool_name: &str) -> Result<PathBuf, String> {
        if let Some(path) = self.resolve_tool(tool_name) {
            Ok(path)
        } else {
            if cfg!(target_os = "linux") {
                let install_cmd = self.get_linux_install_instructions(tool_name);
                Err(format!(
                    "Tool '{}' not found in system PATH.\n\
                    To install on Linux, run:\n  {}",
                    tool_name, install_cmd
                ))
            } else {
                Err(format!("Tool '{}' not found. Please ensure it's installed or bundled with the application.", tool_name))
            }
        }
    }
}

impl Default for ToolPathResolver {
    fn default() -> Self {
        Self::new()
    }
}
