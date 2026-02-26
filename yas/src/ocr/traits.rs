use std::time::Duration;

use anyhow::Result;

pub trait ImageToText<ImageType> {
    fn image_to_text(&self, image: &ImageType, is_preprocessed: bool) -> Result<String>;

    /// Same as image_to_text but with preprocessing tuned for gray (待激活) substat line.
    /// Use for the fourth substat rect; no hardcoded stat names.
    fn image_to_text_pending_line(&self, image: &ImageType) -> Result<String> {
        self.image_to_text(image, false)
    }

    fn get_average_inference_time(&self) -> Option<Duration>;
}

// pub trait ImageTextDetection<ImageType> {
//
// }
