use image::{ImageBuffer, Luma, RgbImage, GenericImageView};
use image::imageops;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Minimum pixel value to consider as "content" when cropping.
const CROP_CONTENT_THRESHOLD: f32 = 0.7;

/// Global binarization: pixel >= threshold -> 1 (text), else 0. Tune for normal lines.
pub const BINARIZE_THRESHOLD: f32 = 0.53;

/// Global binarization threshold for pending (待激活) line; often lower to keep gray as text.
pub const BINARIZE_THRESHOLD_PENDING: f32 = 0.5;

/// convert rgb image to f32 gray image
pub fn to_gray(raw: &RgbImage) -> ImageBuffer<Luma<f32>, Vec<f32>> {
    let mut new_gray: ImageBuffer<Luma<f32>, Vec<f32>> = ImageBuffer::new(raw.width(), raw.height());
    for x in 0..raw.width() {
        for y in 0..raw.height() {
            let rgb = raw.get_pixel(x, y);
            let r = rgb[0] as f32 / 255.0;
            let g = rgb[1] as f32 / 255.0;
            let b = rgb[2] as f32 / 255.0;
            let gray = r * 0.2989 + g * 0.5870 + b * 0.1140;
            let grayp = new_gray.get_pixel_mut(x, y);
            grayp[0] = gray;
        }
    }
    new_gray
}

/// normalize an f32 gray image
fn normalize(im: &mut ImageBuffer<Luma<f32>, Vec<f32>>, auto_inverse: bool) -> bool {
    let width = im.width();
    let height = im.height();
    if width == 0 || height == 0 {
        return false;
    }
    let mut max: f32 = 0.0;
    let mut min: f32 = 256.0;
    for i in 0..width {
        for j in 0..height {
            let p = im.get_pixel(i, j)[0];
            if p > max { max = p; }
            if p < min { min = p; }
        }
    }
    if max == min {
        return false;
    }
    let flag_pixel = if width >= 2 {
        im.get_pixel(width - 2, height - 1)[0]
    } else {
        im.get_pixel(width - 1, height - 1)[0]
    };
    let flag_pixel = (flag_pixel - min) / (max - min);
    for i in 0..width {
        for j in 0..height {
            let p = im.get_pixel_mut(i, j);
            let pv = p[0];
            let mut new_pv = (pv - min) / (max - min);
            if auto_inverse && flag_pixel >= 0.5 {
                new_pv = 1.0 - new_pv;
            }
            p[0] = new_pv;
        }
    }
    true
}

/// crop an f32 gray image to only where there is text
fn crop(im: &ImageBuffer<Luma<f32>, Vec<f32>>) -> ImageBuffer<Luma<f32>, Vec<f32>> {
    let width = im.width();
    let height = im.height();
    let mut min_col = width - 1;
    let mut max_col = 0;
    let mut min_row = height - 1;
    let mut max_row = 0_u32;
    for i in 0..width {
        for j in 0..height {
            let p = im.get_pixel(i, j)[0];
            if p > CROP_CONTENT_THRESHOLD {
                if i < min_col { min_col = i; }
                if i > max_col { max_col = i; }
                break;
            }
        }
    }
    for j in 0..height {
        for i in 0..width {
            let p = im.get_pixel(i, j)[0];
            if p > CROP_CONTENT_THRESHOLD {
                if j < min_row { min_row = j; }
                if j > max_row { max_row = j; }
                break;
            }
        }
    }
    if min_col > max_col || min_row > max_row {
        return im.clone();
    }
    let new_height = max_row - min_row + 1;
    let new_width = max_col - min_col + 1;
    im.view(min_col, min_row, new_width, new_height).to_image()
}

/// resize an f32 gray image to 384 * 32, if not wide enough, then pad with background
fn resize_and_pad(im: &ImageBuffer<Luma<f32>, Vec<f32>>) -> ImageBuffer<Luma<f32>, Vec<f32>> {
    let w = im.width();
    let h = im.height();
    // Hacking: incomplete image seems produce correct result.
    // let new_width = if w as f64 / (h as f64) > 384.0 / 32.0 {
    //     384
    // } else {
    //     std::cmp::min((32.0 / h as f64 * w as f64) as u32, 384)
    // };
    let new_width = w * 32 / h;
    let new_height = 32;
    let img = imageops::resize(im, new_width, new_height, image::imageops::FilterType::Triangle);
    let data: Vec<f32> = vec![0.0; 32 * 384];
    let mut padded_im = ImageBuffer::from_vec(384, 32, data).unwrap();
    imageops::overlay(&mut padded_im, &img, 0, 0);
    padded_im
}

/// Simple global threshold binarization: pixel >= threshold -> 1, else 0.
fn binarize(im: &mut ImageBuffer<Luma<f32>, Vec<f32>>, threshold: f32) {
    for p in im.pixels_mut() {
        p[0] = if p[0] >= threshold { 1.0 } else { 0.0 };
    }
}

static DEBUG_SAVE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// If env YAS_OCR_DEBUG_SAVE=1 (or "true"), save binarized image to dir from YAS_OCR_DEBUG_DIR (default: debug_binarized).
/// Filename: {label}_{count}.png (e.g. normal_0000.png, pending_0001.png). Text = white (255), background = black (0).
pub fn save_binarized_for_debug_if_enabled(im: &ImageBuffer<Luma<f32>, Vec<f32>>, label: &str) {
    let enabled = std::env::var("YAS_OCR_DEBUG_SAVE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let dir = std::env::var("YAS_OCR_DEBUG_DIR").unwrap_or_else(|_| "debug_binarized".into());
    let _ = std::fs::create_dir_all(&dir);
    let n = DEBUG_SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::path::Path::new(&dir).join(format!("{}_{:04}.png", label, n));
    let u8_img: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::from_fn(im.width(), im.height(), |x, y| {
        let v = im.get_pixel(x, y)[0];
        Luma([if v >= 0.5 { 255 } else { 0 }])
    });
    let _ = u8_img.save(&path);
    eprintln!("[YAS OCR] saved binarized debug image: {}", path.display());
}

/// transform an f32 gray image to a preprocessed image
pub fn pre_process(im: ImageBuffer<Luma<f32>, Vec<f32>>) -> (ImageBuffer<Luma<f32>, Vec<f32>>, bool) {
    let mut im = im;
    if !normalize(&mut im, true) {
        return (im, false);
    }
    let mut im = crop(&im);
    normalize(&mut im, false);
    let mut im = resize_and_pad(&im);
    binarize(&mut im, BINARIZE_THRESHOLD);
    save_binarized_for_debug_if_enabled(&im, "normal");
    (im, true)
}

/// Same as `pre_process` but uses BINARIZE_THRESHOLD_PENDING for the fourth substat (e.g. 待激活).
pub fn pre_process_pending_line(im: ImageBuffer<Luma<f32>, Vec<f32>>) -> (ImageBuffer<Luma<f32>, Vec<f32>>, bool) {
    let mut im = im;
    if !normalize(&mut im, true) {
        return (im, false);
    }
    let mut im = crop(&im);
    // println!("shape after first crop: {:?} x {:?}", im.width(), im.height());
    normalize(&mut im, false);
    let mut im = resize_and_pad(&im);
    binarize(&mut im, BINARIZE_THRESHOLD_PENDING);
    save_binarized_for_debug_if_enabled(&im, "pending");
    (im, true)
}
