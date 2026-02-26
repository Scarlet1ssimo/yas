use anyhow::Result;
use clap::Parser;
use image::io::Reader as ImageReader;
use image::RgbImage;
use yas::ocr::{ImageToText, PPOCRChV4RecInfer};
use yas_scanner_genshin::artifact::ArtifactStat;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    image: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize OCR model
    use yas::ocr::yas_ocr_model;
    let model: Box<dyn ImageToText<RgbImage>> = Box::new(yas_ocr_model!(
        "../scanner/artifact_scanner/models/model_training.onnx",
        "../scanner/artifact_scanner/models/index_2_word.json"
    )?);

    // Load image
    println!("Loading image: {}", args.image);
    let image = ImageReader::open(&args.image)?.decode()?;
    let rgb_image = image.to_rgb8();

    // Run OCR
    println!("Running OCR...");
    let result = model.image_to_text(&rgb_image, false)?;
    println!("OCR Result: {}", result);

    // Parse
    println!("Attempting to parse as ArtifactStat...");
    match ArtifactStat::from_zh_cn_raw(&result) {
        Some(stat) => {
            println!("Successfully parsed!");
            println!("Name: {:?}", stat.name);
            println!("Value: {}", stat.value);
            println!("Pending: {}", stat.pending);
        },
        None => {
            println!("Failed to parse as ArtifactStat.");
            println!(
                "Tip: Ensure the image contains a single stat line like '暴击伤害+7.8% (待激活)'."
            );
        },
    }

    Ok(())
}
