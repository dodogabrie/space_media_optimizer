//! Test tool resolver functionality
use space_media_optimizer::tool_resolver::ToolPathResolver;

fn main() {
    env_logger::init();
    
    let resolver = ToolPathResolver::new();
    // println!("Tool resolver report:");
    // println!("{}", resolver.get_tools_report());
    
    // println!("\nChecking specific tools:");
    for tool in &["oxipng", "optipng", "pngcrush", "ffmpeg", "cwebp"] {
        match resolver.resolve_tool(tool) {
            Some(path) => // println!("✅ {} -> {:?}", tool, path),
            None => // println!("❌ {} not found", tool),
        }
    }
}
