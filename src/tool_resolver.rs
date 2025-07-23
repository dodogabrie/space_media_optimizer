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
        // debug!("Current directory: {:?}", current_dir);
        
        // Strategy 1: Look for tools directory by traversing up the directory tree
        let mut search_dir = current_dir.clone();
        for _ in 0..10 { // Max 10 levels up
            let tools_path = search_dir.join("src").join("tools");
            // debug!("Checking tools path: {:?}", tools_path);
            if tools_path.exists() {
                // debug!("Found tools directory by traversing up: {:?}", tools_path);
                return Some(tools_path);
            }
            
            // Go up one level
            if let Some(parent) = search_dir.parent() {
                search_dir = parent.to_path_buf();
            } else {
                break;
            }
        }
        
        // Strategy 2: Check if we can find it via environment variable
        if let Ok(tools_dir) = env::var("OPTIMIZATION_TOOLS_DIR") {
            let tools_path = PathBuf::from(tools_dir);
            // debug!("Checking environment variable path: {:?}", tools_path);
            if tools_path.exists() {
                // debug!("Found tools directory via environment variable: {:?}", tools_path);
                return Some(tools_path);
            }
        }
        
        // Strategy 3: Try to detect Electron app directory structure for production
        if let Ok(exe_path) = env::current_exe() {
            // debug!("Executable path: {:?}", exe_path);
            if let Some(app_dir) = exe_path.parent() {
                // debug!("App directory: {:?}", app_dir);
                
                // Common Electron app structures
                let possible_paths = [
                    app_dir.join("resources").join("app").join("src").join("tools"),
                    app_dir.join("resources").join("app").join("tools"),
                    app_dir.join("tools"),
                    app_dir.join("resources").join("tools"),
                ];

                for path in &possible_paths {
                    // debug!("Checking production path: {:?}", path);
                    if path.exists() {
                        // debug!("Found bundled tools directory: {:?}", path);
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
        // debug!("Resolving tool: {}", tool_name);
        // debug!("Tools directory: {:?}", self.tools_dir);
        
        // Try bundled tools first (for Electron distribution)
        if let Some(ref tools_dir) = self.tools_dir {
            let bundled_path = self.get_bundled_tool_path(tools_dir, tool_name);
            // debug!("Checking bundled path: {:?}", bundled_path);
            if bundled_path.exists() {
                // debug!("Using bundled tool: {} -> {:?}", tool_name, bundled_path);
                return Some(bundled_path);
            } else {
                // debug!("Bundled path does not exist: {:?}", bundled_path);
            }
        } else {
            // debug!("No tools directory configured");
        }

        // Fall back to system PATH
        if let Some(system_path) = self.find_in_system_path(tool_name) {
            // debug!("Using system tool: {} -> {:?}", tool_name, system_path);
            return Some(system_path);
        }

        warn!("Tool not found: {}", tool_name);
        None
    }

    /// Get the expected path for a bundled tool
    fn get_bundled_tool_path(&self, tools_dir: &Path, tool_name: &str) -> PathBuf {
        let platform = env::consts::OS;
        let extension = if platform == "windows" { ".exe" } else { "" };
        
        // Platform-specific tool organization
        tools_dir
            .join(platform)
            .join(tool_name)
            .join(format!("{}{}", tool_name, extension))
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

    /// Get a report of tool availability
    pub fn get_tools_report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!("Tool Path Resolver Report\n"));
        report.push_str(&format!("Development mode: {}\n", self.is_development));
        report.push_str(&format!("Bundled tools dir: {:?}\n", self.tools_dir));
        report.push_str("\nTool Availability:\n");

        let tools = [
            ("WebP", vec!["cwebp", "dwebp"]),
            ("JPEG", vec!["mozjpeg", "jpegoptim", "jpegtran"]),
            ("PNG", vec!["oxipng", "optipng", "pngcrush"]),
            ("Video", vec!["ffmpeg", "ffprobe"]),
            ("Metadata", vec!["exiftool"]),
        ];

        for (category, tool_list) in tools {
            report.push_str(&format!("\n{}:\n", category));
            for tool in tool_list {
                let status = if let Some(path) = self.resolve_tool(tool) {
                    format!("✅ {} -> {:?}", tool, path)
                } else {
                    format!("❌ {} (not found)", tool)
                };
                report.push_str(&format!("  {}\n", status));
            }
        }

        report
    }
}

impl Default for ToolPathResolver {
    fn default() -> Self {
        Self::new()
    }
}
