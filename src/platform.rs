//! # Platform-specific utilities
//!
//! Questo modulo centralizza tutta la logica per la gestione cross-platform
//! dei comandi e delle dipendenze esterne. Supporta tool bundled in Electron
//! e tool di sistema.

use std::collections::HashMap;
use std::sync::OnceLock;
use crate::tool_resolver::ToolPathResolver;

/// Platform-specific command manager with tool resolution
pub struct PlatformCommands {
    commands: HashMap<&'static str, &'static str>,
    which_command: &'static str,
    tool_resolver: ToolPathResolver,
}

impl PlatformCommands {
    /// Get the singleton instance
    pub fn instance() -> &'static Self {
        static INSTANCE: OnceLock<PlatformCommands> = OnceLock::new();
        INSTANCE.get_or_init(|| Self::new())
    }
    
    /// Initialize platform-specific commands
    fn new() -> Self {
        let (commands, which_command) = if cfg!(windows) {
            // Windows commands
            let mut commands = HashMap::new();
            commands.insert("exiftool", "exiftool.exe");
            commands.insert("cwebp", "cwebp.exe");
            commands.insert("ffmpeg", "ffmpeg.exe");
            commands.insert("ffprobe", "ffprobe.exe");
            commands.insert("mozjpeg", "mozjpeg.exe");
            commands.insert("jpegoptim", "jpegoptim.exe");
            commands.insert("jpegtran", "jpegtran.exe");
            commands.insert("oxipng", "oxipng.exe");
            commands.insert("optipng", "optipng.exe");
            commands.insert("pngcrush", "pngcrush.exe");
            (commands, "where")
        } else {
            // Unix-like systems (Linux, macOS)
            let mut commands = HashMap::new();
            commands.insert("exiftool", "exiftool");
            commands.insert("cwebp", "cwebp");
            commands.insert("ffmpeg", "ffmpeg");
            commands.insert("ffprobe", "ffprobe");
            commands.insert("mozjpeg", "mozjpeg");
            commands.insert("jpegoptim", "jpegoptim");
            commands.insert("jpegtran", "jpegtran");
            commands.insert("oxipng", "oxipng");
            commands.insert("optipng", "optipng");
            commands.insert("pngcrush", "pngcrush");
            (commands, "which")
        };

        Self {
            commands,
            which_command,
            tool_resolver: ToolPathResolver::new(),
        }
    }
    
    /// Get the platform-specific command name
    pub fn get_command<'a>(&self, base_name: &'a str) -> &'a str {
        self.commands.get(base_name).unwrap_or(&base_name)
    }
    
    /// Get the command used to check if a program exists
    pub fn which_command(&self) -> &str {
        self.which_command
    }
    
    /// Check if a command is available on the system or bundled
    pub async fn is_command_available(&self, base_name: &str) -> bool {
        // First try the tool resolver (bundled tools + system PATH)
        if self.tool_resolver.is_tool_available(base_name) {
            return true;
        }

        // Fallback to traditional which/where command
        let command_name = self.get_command(base_name);
        
        // Use tokio::process::Command for async execution
        let result = tokio::process::Command::new(self.which_command)
            .arg(command_name)
            .output()
            .await;
            
        match result {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Get the resolved path to a tool (bundled or system)
    pub fn get_tool_path(&self, base_name: &str) -> Option<std::path::PathBuf> {
        self.tool_resolver.resolve_tool(base_name)
    }

    /// Get a report of all available tools
    pub fn get_tools_report(&self) -> String {
        self.tool_resolver.get_tools_report()
    }
    
    /// Get system information for // debugging
    pub fn system_info() -> SystemInfo {
        SystemInfo {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            family: std::env::consts::FAMILY,
        }
    }
}

/// System information structure
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os: &'static str,
    pub arch: &'static str,
    pub family: &'static str,
}

impl std::fmt::Display for SystemInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} ({})", self.os, self.arch, self.family)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_platform_commands() {
        let platform = PlatformCommands::instance();
        
        // Test that we get some command back
        let exiftool = platform.get_command("exiftool");
        assert!(!exiftool.is_empty());
        
        // Test which command
        let which = platform.which_command();
        assert!(!which.is_empty());
    }
    
    #[tokio::test]
    async fn test_command_availability() {
        let platform = PlatformCommands::instance();
        
        // Test with a command that should exist on most systems
        let has_echo = platform.is_command_available("echo").await;
        // Don't assert true because it might not exist in some minimal environments
        // Just ensure the function doesn't panic
        let _ = has_echo;
    }
    
    #[test]
    fn test_system_info() {
        let info = PlatformCommands::system_info();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.family.is_empty());
    }
}
