use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

fn main() {
    // Только для Windows
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let ico_path = Path::new(&out_dir).join("icon.ico");
    
    // Конвертируем PNG в ICO
    if let Err(e) = create_ico(&ico_path) {
        println!("cargo:warning=Failed to create ICO: {}", e);
        return;
    }

    // Встраиваем иконку в exe
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    res.set("ProductName", "ByStep Launcher");
    res.set("FileDescription", "Minecraft Launcher");
    res.set("LegalCopyright", "ByStep");
    
    if let Err(e) = res.compile() {
        println!("cargo:warning=Failed to compile resources: {}", e);
    }
}

fn create_ico(ico_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let png_path = Path::new("src/icon.png");
    
    if !png_path.exists() {
        return Err("icon.png not found in src/".into());
    }

    let img = image::open(png_path)?;
    
    // Создаём ICO с разными размерами
    let sizes = [256, 128, 64, 48, 32, 16];
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    
    for size in sizes {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        
        let icon_image = ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
        icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image)?);
    }
    
    let file = File::create(ico_path)?;
    icon_dir.write(BufWriter::new(file))?;
    
    Ok(())
}
