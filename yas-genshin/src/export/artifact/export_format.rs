use clap::ValueEnum;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum GenshinArtifactExportFormat {
    Mona,
    #[value(name = "mona_extended")]
    MonaExtended,
    MingyuLab,
    Good,
    CSV,
    /// Export all formats
    All,
}

impl Default for GenshinArtifactExportFormat {
    fn default() -> Self {
        Self::Mona
    }
}
