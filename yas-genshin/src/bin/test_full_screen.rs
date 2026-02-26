//! 用一张全窗口截图测试圣遗物识别，直接复用 ArtifactScannerWorker 的扣图与 OCR 流程。
//!
//! ## 分辨率与 Scale 运作方式
//!
//! - **window_info JSON** 里所有坐标/尺寸都是针对某一基准分辨率（如 1600x900）标定的。
//! - **Repository** 按 (width, height, ui, platform) 存多套配置；`get_auto_scale` 在宽高比一致时
//!   用 `factor = target_width / base_width` 对 Rect/Pos/Size/Float 做线性缩放，**InvariantInt/InvariantFloat 不缩放**。
//! - **from_window_info_repository** 根据当前窗口尺寸取配置（或同比例缩放），得到的是**已换算到当前分辨率**的 rect/pos，单位是像素。
//!
//! ## 新标点（新 ROI）要不要做坐标变换？
//!
//! - **要**：新标点若在 JSON 里是某一基准分辨率下的坐标，必须和现有项一样参与同一套 scale 逻辑。
//! - 即：在对应 resolution 的 JSON 里用 **Rect/Pos/Size** 标好基准坐标，运行时由 `FromWindowInfoRepository` 按当前窗口尺寸自动缩放；**不要**在代码里手写像素或对“新标点”单独做变换。
//! - 若某值是“与分辨率无关”的常数（如次数、枚举），用 **InvariantInt** / **InvariantFloat**，不会随分辨率缩放。

use anyhow::{Context, Result};
use clap::Parser;
use image::io::Reader as ImageReader;
use image::{GenericImageView, RgbImage};
use yas::game_info::{Platform, UI};
use yas::positioning::Size;
use yas::window_info::{load_window_info_repo, FromWindowInfoRepository};
use yas_scanner_genshin::scanner::{
    get_page_locks_from_list_image, ArtifactScannerWindowInfo, ArtifactScannerWorker,
    GenshinArtifactScannerConfig,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    image: String,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    // 1. Load full-window image
    println!("Loading image: {}", args.image);
    let image = ImageReader::open(&args.image)?.decode()?;
    let rgb_image = image.to_rgb8();
    let (width, height) = rgb_image.dimensions();
    println!("Image dimensions: {}x{}", width, height);

    // 2. Window info repo (same as application) and scale to current image size
    let repo = load_window_info_repo!(
        "../../window_info/windows1600x900.json",
        "../../window_info/windows1280x960.json",
        "../../window_info/windows1440x900.json",
        "../../window_info/windows2100x900.json",
        "../../window_info/windows3440x1440.json",
    );

    let target_size = Size {
        width: width as usize,
        height: height as usize,
    };
    let window_info = ArtifactScannerWindowInfo::from_window_info_repository(
        target_size,
        UI::Desktop,
        Platform::Windows,
        &repo,
    )
    .with_context(|| format!("No window info for size {}x{}", width, height))?;

    println!("Window info loaded and scaled to {}x{}", width, height);
    println!("Panel rect: {:?}", window_info.panel_rect);

    // 3. Crop panel from full-window image (same coordinate system: (0,0) = top-left of window)
    let pr = &window_info.panel_rect;
    let px = pr.left as u32;
    let py = pr.top as u32;
    let pw = pr.width as u32;
    let ph = pr.height as u32;

    if px + pw > width || py + ph > height {
        anyhow::bail!(
            "Panel rect ({}, {}, {}, {}) out of image bounds {}x{}",
            px, py, pw, ph,
            width, height
        );
    }

    let panel_image: RgbImage = rgb_image.view(px, py, pw, ph).to_image();
    panel_image.save("panel_image.png")?;
    println!("Cropped panel size: {}x{}", panel_image.width(), panel_image.height());

    // 3b. Crop list region (first page: from scan_margin_pos, full remaining size) and get lock 0/1 table
    let margin = window_info.scan_margin_pos;
    let list_left = margin.x as u32;
    let list_top = margin.y as u32;
    if list_left < width && list_top < height {
        let list_w = width - list_left;
        let list_h = height - list_top;
        let list_image: RgbImage = rgb_image.view(list_left, list_top, list_w, list_h).to_image();
        let locks = get_page_locks_from_list_image(&list_image, &window_info, Some(std::path::Path::new("debug")));
        let row = window_info.row;
        let col = window_info.col;
        println!("\n=== List-view lock state (0=unlocked, 1=locked), row x col ===");
        for r in 0..row {
            for c in 0..col {
                let i = (r * col + c) as usize;
                if i < locks.len() {
                    print!("{}", if locks[i] { 1 } else { 0 });
                }
            }
            println!();
        }
    }

    // 4. Use the same worker as the live scanner: one call does all rects + OCR
    let config = GenshinArtifactScannerConfig {
        min_star: 4,
        min_level: 0,
        ignore_dup: false,
        verbose: true,
        number: -1,
        lock_list_path: None,
    };
    let worker = ArtifactScannerWorker::new(window_info, config)?;
    let result = worker.scan_panel_image(&panel_image, false)?;

    println!("\n=== Scan result ===");
    println!("{:#?}", result);

    Ok(())
}
