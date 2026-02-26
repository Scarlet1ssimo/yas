//! Lock list: JSON of artifacts to auto-lock when scanning. Match by name, main_stat, sub_stat (mona_extended compatible).
//!
//! JSON format: array of objects, each with `name`, `main_stat_name`, `main_stat_value`, `sub_stat` (array of 4 strings).
//! Example:
//! ```json
//! [
//!   {
//!     "name": "杰作的序曲",
//!     "main_stat_name": "攻击力",
//!     "main_stat_value": "311",
//!     "sub_stat": ["生命值+15.2%", "暴击伤害+7.8%", "防御力+65", "元素精通+35"]
//!   }
//! ]
//! ```

use serde::Deserialize;

use super::scan_result::GenshinArtifactScanResult;

/// One artifact entry in the lock list. Matches scan result by name, main_stat_name, main_stat_value, sub_stat.
#[derive(Debug, Clone, Deserialize)]
pub struct LockListEntry {
    pub name: String,
    pub main_stat_name: String,
    pub main_stat_value: String,
    /// Exactly 4 substat strings, order matters.
    pub sub_stat: [String; 4],
}

/// Lock list JSON: array of artifacts to lock (mona_extended-style keys).
#[derive(Debug, Clone, Deserialize)]
pub struct LockList(pub Vec<LockListEntry>);

impl LockList {
    pub fn from_json_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("read lock list: {}", path.display()))?;
        let list: Vec<LockListEntry> = serde_json::from_str(&s)
            .with_context(|| "parse lock list JSON (expected array of { name, main_stat_name, main_stat_value, sub_stat: [4] })")?;
        Ok(LockList(list))
    }

    /// True if this scan result exactly matches one entry (name, main_stat_name, main_stat_value, sub_stat).
    pub fn contains(&self, r: &GenshinArtifactScanResult) -> bool {
        self.0.iter().any(|e| {
            e.name.trim() == r.name.trim()
                && e.main_stat_name.trim() == r.main_stat_name.trim()
                && e.main_stat_value.trim() == r.main_stat_value.trim()
                && e.sub_stat[0].trim() == r.sub_stat[0].trim()
                && e.sub_stat[1].trim() == r.sub_stat[1].trim()
                && e.sub_stat[2].trim() == r.sub_stat[2].trim()
                && e.sub_stat[3].trim() == r.sub_stat[3].trim()
        })
    }
}

// for with_context
use anyhow::Context;
