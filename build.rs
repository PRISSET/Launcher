use std::env;
use std::path::Path;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let ico_path = Path::new("icon.ico");
    
    if !ico_path.exists() {
        println!("cargo:warning=icon.ico not found, skipping icon embedding");
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    res.set("ProductName", "ByStep Launcher");
    res.set("FileDescription", "Minecraft Launcher");
    res.set("LegalCopyright", "ByStep");
    
    if let Err(e) = res.compile() {
        println!("cargo:warning=Failed to compile resources: {}", e);
    }
}
