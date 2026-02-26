use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use image::Rgb;
use image::{GenericImageView, RgbImage};
use log::{error, info, warn};

use yas::ocr::yas_ocr_model;
use yas::ocr::ImageToText;
use yas::positioning::{Pos, Rect};
use yas::utils::color_distance;

use crate::artifact::ArtifactStat;
use crate::scanner::artifact_scanner::artifact_scanner_window_info::ArtifactScannerWindowInfo;
use crate::scanner::artifact_scanner::message_items::SendItem;
use crate::scanner::artifact_scanner::scan_result::GenshinArtifactScanResult;
use crate::scanner::artifact_scanner::GenshinArtifactScannerConfig;

fn parse_level(s: &str) -> Result<i32> {
    let pos = s.find('+');

    if pos.is_none() {
        let level = s
            .parse::<i32>()
            .with_context(|| format!("parse level (OCR raw: {:?})", s))?;
        return Ok(level);
    }

    let level = s[pos.unwrap()..]
        .parse::<i32>()
        .with_context(|| format!("parse level after '+' (OCR raw: {:?})", s))?;
    Ok(level)
}

/// List-view lock detection: from a cropped list grid image, return lock state per cell (row-major).
/// Uses lock icon color [255,138,117] at lock_pos within each cell. Caller crops the list region
/// (e.g. scan_margin_pos + first-page rect) from the window image.
/// If `debug_dir` is Some, dumps the sampled pixel region image for each (row, col) as `r{row}_c{col}.png`.
pub fn get_page_locks_from_list_image(
    list_image: &RgbImage,
    window_info: &ArtifactScannerWindowInfo,
    debug_dir: Option<&Path>,
) -> Vec<bool> {
    let mut result = Vec::new();
    let row = window_info.row;
    let col = window_info.col;
    let gap = window_info.item_gap_size;
    let size = window_info.item_size;
    let lock_pos = Pos { x: 19.0, y: 93.0 };
    
    if let Some(dir) = debug_dir {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join(format!("qwq.png"));
        let _ = list_image.save(&path);
    }

    for r in 0..row {
        if ((gap.height + size.height) * (r as f64)) as u32 > list_image.height() {
            break;
        }
        for c in 0..col {
            let pos_x = (gap.width + size.width) * (c as f64) + lock_pos.x;
            let pos_y = (gap.height + size.height) * (r as f64) + lock_pos.y;
            println!("pos_x: {}, pos_y: {}", pos_x, pos_y);

            if let Some(dir) = debug_dir {
                let px = pos_x as i32;
                let py = pos_y as i32;
                let left = px.saturating_sub(2).max(0) as u32;
                let top = py.saturating_sub(10).max(0) as u32;
                let w = (list_image.width()).saturating_sub(left).min(6);
                let h = (list_image.height()).saturating_sub(top).min(22);
                if w > 0 && h > 0 {
                    let crop = list_image.view(left, top, w, h).to_image();
                    let path = dir.join(format!("r{}_c{}.png", r, c));
                    let _ = crop.save(&path);
                }
            }

            let mut locked = false;
            'sq: for dx in -1..1 {
                for dy in -10..10 {
                    if pos_y as i32 + dy < 0
                        || (pos_y as i32 + dy) as u32 >= list_image.height()
                    {
                        continue;
                    }

                    let color = list_image.get_pixel(
                        (pos_x as i32 + dx) as u32,
                        (pos_y as i32 + dy) as u32,
                    );

                    if color_distance(color, &Rgb([255, 138, 117])) < 30 {
                        locked = true;
                        break 'sq;
                    }
                }
            }
            result.push(locked);
        }
    }
    result
}

fn get_image_to_text() -> Result<Box<dyn ImageToText<RgbImage> + Send>> {
    use yas::ocr::yas_ocr_model;
    let model: Box<dyn ImageToText<RgbImage> + Send> = Box::new(yas_ocr_model!(
        "./models/model_training.onnx",
        "./models/index_2_word.json"
    )?);
    Ok(model)
}

/// run in a separate thread, accept captured image and get an artifact
pub struct ArtifactScannerWorker {
    model: Box<dyn ImageToText<RgbImage> + Send>,
    window_info: ArtifactScannerWindowInfo,
    config: GenshinArtifactScannerConfig,
}

impl ArtifactScannerWorker {
    pub fn new(
        window_info: ArtifactScannerWindowInfo,
        config: GenshinArtifactScannerConfig,
    ) -> Result<Self> {
        Ok(ArtifactScannerWorker {
            model: get_image_to_text()?,
            window_info,
            config,
        })
    }

    /// the captured_img is a panel of the artifact, the rect is a region of the panel
    fn model_inference(&self, rect: Rect<f64>, captured_img: &RgbImage) -> Result<String> {
        let relative_rect = rect.translate(Pos {
            x: -self.window_info.panel_rect.left,
            y: -self.window_info.panel_rect.top,
        });

        let w = captured_img.width();
        let h = captured_img.height();
        let x = relative_rect.left as u32;
        let y = relative_rect.top as u32;
        let rw = relative_rect.width as u32;
        let rh = relative_rect.height as u32;
        if x.saturating_add(rw) > w || y.saturating_add(rh) > h {
            anyhow::bail!(
                "crop region out of bounds: rect ({}..{}, {}..{}) vs image {}x{}",
                x,
                x + rw,
                y,
                y + rh,
                w,
                h
            );
        }

        let raw_img = captured_img.view(x, y, rw, rh).to_image();
        self.model.image_to_text(&raw_img, false)
    }

    /// Same as model_inference but with preprocessing tuned for gray (待激活) substat; use for 4th substat.
    fn model_inference_pending_line(&self, rect: Rect<f64>, captured_img: &RgbImage) -> Result<String> {
        let relative_rect = rect.translate(Pos {
            x: -self.window_info.panel_rect.left,
            y: -self.window_info.panel_rect.top,
        });
        let w = captured_img.width();
        let h = captured_img.height();
        let x = relative_rect.left as u32;
        let y = relative_rect.top as u32;
        let rw = relative_rect.width as u32;
        let rh = relative_rect.height as u32;
        if x.saturating_add(rw) > w || y.saturating_add(rh) > h {
            anyhow::bail!(
                "crop region out of bounds (pending-line): rect ({}..{}, {}..{}) vs image {}x{}",
                x,
                x + rw,
                y,
                y + rh,
                w,
                h
            );
        }
        let raw_img = captured_img.view(x, y, rw, rh).to_image();
        self.model.image_to_text_pending_line(&raw_img)
    }

    /// Check if the panel image has the purple 祝圣之霜 (Blessed Frost) block in the configured detect rect.
    /// Uses color distance to a reference purple; when enough pixels match, returns true.
    fn has_blessed_frost_mark(&self, panel_image: &RgbImage) -> bool {
        if self.window_info.blessed_frost_detect_rect.height <= 0.0 {
            return false;
        }
        let relative_rect = self.window_info.blessed_frost_detect_rect.translate(Pos {
            x: -self.window_info.panel_rect.left,
            y: -self.window_info.panel_rect.top,
        });
        let left = relative_rect.left as u32;
        let top = relative_rect.top as u32;
        let w = relative_rect.width as u32;
        let h = relative_rect.height as u32;
        if left + w > panel_image.width() || top + h > panel_image.height() {
            return false;
        }
        // Reference purple for 祝圣之霜 block (RGB)
        const PURPLE_REF: Rgb<u8> = Rgb([220, 192, 255]);
        const DIST_THRESHOLD: usize = 10;
        const RATIO_THRESHOLD: f32 = 0.9;
        let mut match_count = 0u32;
        let total = (w * h).max(1);
        for py in top..(top + h) {
            for px in left..(left + w) {
                let color = panel_image.get_pixel(px, py);
                if color_distance(color, &PURPLE_REF) < DIST_THRESHOLD {
                    match_count += 1;
                }
            }
        }
        (match_count as f32 / total as f32) >= RATIO_THRESHOLD
    }

    /// Scan a single panel image (e.g. cropped from a full window screenshot).
    /// Use this when you already have the artifact panel image and want to run the same
    /// inference pipeline as the live scanner (e.g. from test_full_screen).
    pub fn scan_panel_image(
        &self,
        panel_image: &RgbImage,
        lock: bool,
    ) -> Result<GenshinArtifactScanResult> {
        let item = SendItem {
            panel_image: panel_image.clone(),
            list_image: None,
            star: 0,
        };
        self.scan_item_image(item, lock)
    }

    /// Parse the captured result (of type SendItem) to a scanned artifact
    fn scan_item_image(&self, item: SendItem, lock: bool) -> Result<GenshinArtifactScanResult> {
        let image = &item.panel_image;

        let str_title = self
            .model_inference(self.window_info.title_rect, image)
            .context("OCR title_rect")?;
        let str_main_stat_name = self
            .model_inference(self.window_info.main_stat_name_rect, image)
            .context("OCR main_stat_name_rect")?;
        let str_main_stat_value = self
            .model_inference(self.window_info.main_stat_value_rect, image)
            .context("OCR main_stat_value_rect")?;

        let offset_y = if self.window_info.blessed_frost_offset_y != 0.0
            && self.has_blessed_frost_mark(image)
        {
            println!("Blessed Frost detected");
            self.window_info.blessed_frost_offset_y
        } else {
            0.0
        };
        let offset = Pos { x: 0.0, y: offset_y };

        // When 祝圣之霜 block is present, shift level and sub_stats rects down (item_equip_rect stays)
        let level_rect = self.window_info.level_rect.translate(offset);
        let sub_stat_1 = self.window_info.sub_stat_1.translate(offset);
        let sub_stat_2 = self.window_info.sub_stat_2.translate(offset);
        let sub_stat_3 = self.window_info.sub_stat_3.translate(offset);
        let sub_stat_4 = self.window_info.sub_stat_4.translate(offset);

        let str_sub_stat0 = self
            .model_inference(sub_stat_1, image)
            .context("OCR sub_stat_1")?;
        let str_sub_stat1 = self
            .model_inference(sub_stat_2, image)
            .context("OCR sub_stat_2")?;
        let str_sub_stat2 = self
            .model_inference(sub_stat_3, image)
            .context("OCR sub_stat_3")?;
        // Fourth substat may be gray (待激活): try normal OCR first; if it doesn't parse, retry with pending-line preprocess
        let str_sub_stat3 = {
            let normal = self
                .model_inference(sub_stat_4, image)
                .context("OCR sub_stat_4")?;
            if ArtifactStat::from_zh_cn_raw(&normal).is_some() {
                normal
            } else {
                self.model_inference_pending_line(sub_stat_4, image)
                    .context("OCR sub_stat_4 (pending-line)")?
            }
        };

        let str_level = self
            .model_inference(level_rect, image)
            .context("OCR level_rect")?;
        let str_equip = self
            .model_inference(self.window_info.item_equip_rect, image)
            .context("OCR item_equip_rect")?;

        let level = parse_level(&str_level).context("parse level from OCR")?;

        Ok(GenshinArtifactScanResult {
            name: str_title,
            main_stat_name: str_main_stat_name,
            main_stat_value: str_main_stat_value,
            sub_stat: [str_sub_stat0, str_sub_stat1, str_sub_stat2, str_sub_stat3],
            level,
            equip: str_equip,
            star: item.star as i32,
            lock,
        })
    }

    /// Get all lock state from a list image (list-view grid). Used for auto-lock: only click lock when list says not locked.
    fn get_page_locks(&self, list_image: &RgbImage) -> Vec<bool> {
        get_page_locks_from_list_image(list_image, &self.window_info, None)
    }

    /// Run the worker. If `result_tx` is Some, send each scan result (or None on error) so the main thread can e.g. auto-lock.
    pub fn run(
        self,
        rx: Receiver<Option<SendItem>>,
        result_tx: Option<Sender<Option<GenshinArtifactScanResult>>>,
    ) -> JoinHandle<Vec<GenshinArtifactScanResult>> {
        std::thread::spawn(move || {
            let mut results = Vec::new();
            let mut hash: HashSet<GenshinArtifactScanResult> = HashSet::new();
            let mut consecutive_dup_count = 0;
            let is_verbose = self.config.verbose;
            let min_level = self.config.min_level;
            let info = self.window_info.clone();
            let mut locks = Vec::new();
            let mut artifact_index: i32 = 0;

            let send_result = |tx: &Option<Sender<Option<GenshinArtifactScanResult>>>, r: Option<GenshinArtifactScanResult>| {
                if let Some(t) = tx {
                    let _ = t.send(r);
                }
            };

            for item in rx.into_iter() {
                let item = match item {
                    Some(v) => v,
                    None => break,
                };

                match item.list_image.as_ref() {
                    Some(v) => locks = vec![locks, self.get_page_locks(v)].concat(),
                    None => {},
                };

                artifact_index += 1;
                let result = match self.scan_item_image(item, locks[artifact_index as usize - 1]) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("识别错误: {}", e);
                        send_result(&result_tx, None);
                        continue;
                    },
                };

                send_result(&result_tx, Some(result.clone()));

                if is_verbose {
                    info!("{:?}", result);
                }

                if result.level < min_level {
                    info!(
                        "找到满足最低等级要求 {} 的物品({})，准备退出……",
                        min_level, result.level
                    );
                    break;
                }

                if hash.contains(&result) {
                    consecutive_dup_count += 1;
                    warn!("识别到重复物品: {:#?}", result);
                } else {
                    consecutive_dup_count = 0;
                    hash.insert(result.clone());
                    results.push(result);
                }

                if consecutive_dup_count >= info.col && !self.config.ignore_dup {
                    error!("识别到连续多个重复物品，可能为翻页错误，或者为非背包顶部开始扫描");
                    break;
                }
            }

            info!("识别结束，非重复物品数量: {}", hash.len());
            results
        })
    }
}
