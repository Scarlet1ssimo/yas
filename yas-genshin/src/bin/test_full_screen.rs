use anyhow::Result;
use clap::Parser;
use image::io::Reader as ImageReader;
use image::{GenericImageView, RgbImage};
use std::fs;
use std::path::PathBuf;
use yas::game_info::{GameInfo, Platform, UI};
use yas::ocr::{ImageToText, PPOCRChV4RecInfer};
use yas::positioning::{Pos, Rect, Size};
use yas::window_info::{WindowInfoRepository, WindowInfoType, FromWindowInfoRepository};
use yas_scanner_genshin::artifact::ArtifactStat;
use yas_scanner_genshin::scanner::ArtifactScannerWindowInfo;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    image: String,
}

fn main() -> Result<()> {
    // Enable logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    
    // 1. Load Image
    println!("Loading image: {}", args.image);
    let image = ImageReader::open(&args.image)?.decode()?;
    let rgb_image = image.to_rgb8();
    let (width, height) = rgb_image.dimensions();
    println!("Image dimensions: {}x{}", width, height);

    // 2. Setup Window Info
    // We need to match the logic in `GenshinArtifactScanner::new`
    // but simplified for static image.
    let mut repo = WindowInfoRepository::new();
    
    // Load 1600x900 config manually since we are in a test script
    let config_path = PathBuf::from("window_info/windows1600x900.json");
    let content = fs::read_to_string(config_path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;
    
    let resolution = config["current_resolution"].as_object().unwrap();
    let base_width = resolution["width"].as_u64().unwrap() as usize;
    let base_height = resolution["height"].as_u64().unwrap() as usize;
    let base_size = Size { width: base_width, height: base_height };
    
    let data = config["data"].as_object().unwrap();
    for (k, v) in data {
        // Simple parser for the specific JSON format in window_info
        let value = if let Some(rect) = v.get("Rect") {
             WindowInfoType::Rect(Rect {
                left: rect["left"].as_f64().unwrap(),
                top: rect["top"].as_f64().unwrap(),
                width: rect["width"].as_f64().unwrap(),
                height: rect["height"].as_f64().unwrap(),
            })
        } else if let Some(pos) = v.get("Pos") {
            WindowInfoType::Pos(Pos {
                x: pos["x"].as_f64().unwrap(),
                y: pos["y"].as_f64().unwrap(),
            })
        } else if let Some(size) = v.get("Size") {
             WindowInfoType::Size(Size {
                width: size["width"].as_f64().unwrap(),
                height: size["height"].as_f64().unwrap(),
            })
        } else if let Some(i) = v.get("InvariantInt") {
            WindowInfoType::InvariantInt(i.as_i64().unwrap() as i32)
        } else {
            continue;
        };
        
        repo.add(k, base_size, UI::Desktop, Platform::Windows, value);
    }

    // 3. Create ArtifactScannerWindowInfo (Auto-scaled)
    // Assuming Desktop/Windows
    let target_size = Size { width: width as usize, height: height as usize };
    let window_info = ArtifactScannerWindowInfo::from_window_info_repository(
        target_size,
        UI::Desktop,
        Platform::Windows,
        &repo
    )?;

    println!("Window Info Loaded and Scaled.");
    println!("Panel Rect: {:?}", window_info.panel_rect);
    println!("Substat 1 Rect: {:?}", window_info.sub_stat_1);

    // 4. Crop and OCR
    let model: Box<dyn ImageToText<RgbImage>> = Box::new(PPOCRChV4RecInfer::new()?);

    let panel_origin_x = 0.0; // Capturer usually returns relative to window, but here we have full window image
    let panel_origin_y = 0.0;

    // Helper to process a rect
    let process_rect = |name: &str, rect: Rect<f64>| -> Result<()> {
        println!("\nProcessing {}...", name);
        // Rect in WindowInfo is relative to the Window (0,0)
        let x = rect.left as u32;
        let y = rect.top as u32;
        let w = rect.width as u32;
        let h = rect.height as u32;
        
        println!("Crop region: x={}, y={}, w={}, h={}", x, y, w, h);
        
        if x + w > width || y + h > height {
            println!("! Rect out of bounds !");
            return Ok(());
        }

        let cropped = rgb_image.view(x, y, w, h).to_image();
        
        // Save cropped for debug
        // cropped.save(format!("debug_{}.png", name))?;
        
        let text = model.image_to_text(&cropped, false)?;
        println!("OCR Result: '{}'", text);
        
        if let Some(stat) = ArtifactStat::from_zh_cn_raw(&text) {
             println!("parsed: {:?}", stat);
        } else {
             println!("failed to parse as stat");
        }
        Ok(())
    };

    process_rect("Title", window_info.title_rect)?;
    process_rect("Main Stat", window_info.main_stat_value_rect)?;
    
    process_rect("SubStat 1", window_info.sub_stat_1)?;
    process_rect("SubStat 2", window_info.sub_stat_2)?;
    process_rect("SubStat 3", window_info.sub_stat_3)?;
    process_rect("SubStat 4", window_info.sub_stat_4)?;

    Ok(())
}

// Minimal stub for FromWindowInfoRepository trait if needed, 
// but it should be available from `yas::window_info::FromWindowInfoRepository`
