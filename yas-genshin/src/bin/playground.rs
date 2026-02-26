use anyhow::Result;
use image::io::Reader as ImageReader;
use yas::ocr::yas_ocr_model;
use yas::ocr::ImageToText;

fn main() -> Result<()> {
    // Verification of custom model loading
    println!("Loading custom model...");

    let model = yas_ocr_model!(
        "../scanner/artifact_scanner/models/model_training.onnx",
        "../scanner/artifact_scanner/models/index_2_word.json"
    )?;

    let path = r"QQ图片20260209170604.png";
    println!("Reading image from {}...", path);
    // Handle image reading error gracefully or let it simple fail
    let image = match ImageReader::open(path) {
        Ok(reader) => reader.decode()?,
        Err(e) => {
            println!("Failed to open image: {}", e);
            return Ok(());
        },
    };
    let rgb_image = image.to_rgb8();
    println!("Running inference...");
    let result = model.image_to_text(&rgb_image, false)?;
    println!("Result: {}", result);

    Ok(())
}
