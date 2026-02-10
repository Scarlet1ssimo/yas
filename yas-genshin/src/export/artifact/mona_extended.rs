use std::convert::From;

use serde::ser::{Serialize, SerializeMap, Serializer};

use crate::artifact::{
    ArtifactSetName, ArtifactSlot, ArtifactStat, ArtifactStatName, GenshinArtifact,
};

use super::mona_uranai::MonaFormat; // Re-use utilities if possible, or just copy logic

// Wrapper for ArtifactStat to support pending field
struct MonaExtendedStat<'a>(&'a ArtifactStat);

impl<'a> Serialize for MonaExtendedStat<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(None)?;
        root.serialize_entry("name", &self.0.name.to_mona()).unwrap();
        root.serialize_entry("value", &self.0.value).unwrap();
        if self.0.pending {
            root.serialize_entry("pending", &true).unwrap();
        }
        root.end()
    }
}

// Wrapper for GenshinArtifact
pub struct MonaExtendedArtifact<'a>(&'a GenshinArtifact);

impl<'a> Serialize for MonaExtendedArtifact<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let artifact = self.0;
        let mut root = serializer.serialize_map(None)?;

        root.serialize_entry("setName", &artifact.set_name.to_mona()).unwrap();
        root.serialize_entry("position", &artifact.slot.to_mona()).unwrap();
        root.serialize_entry("mainTag", &artifact.main_stat).unwrap(); // ArtifactStat has default impl which is fine for main stat?
        // Wait, main stat definitely doesn't have pending in the original implementation, but ArtifactStat's Serialize impl ignores pending.
        // If we want pending on main stat (unlikely but possible), we should use wrapper. 
        // But main stats usually don't have pending. Let's keep mainTag as is for now unless requested.

        let mut sub_stats: Vec<MonaExtendedStat> = vec![];
        if let Some(ref s) = artifact.sub_stat_1 {
            sub_stats.push(MonaExtendedStat(s));
        }
        if let Some(ref s) = artifact.sub_stat_2 {
            sub_stats.push(MonaExtendedStat(s));
        }
        if let Some(ref s) = artifact.sub_stat_3 {
            sub_stats.push(MonaExtendedStat(s));
        }
        if let Some(ref s) = artifact.sub_stat_4 {
            sub_stats.push(MonaExtendedStat(s));
        }

        root.serialize_entry("normalTags", &sub_stats)?;
        root.serialize_entry("omit", &false)?;
        root.serialize_entry("level", &artifact.level)?;
        root.serialize_entry("star", &artifact.star)?;
        root.serialize_entry("equip", &artifact.equip)?;

        root.end()
    }
}

pub struct MonaExtendedFormat<'a> {
    version: String,
    flower: Vec<MonaExtendedArtifact<'a>>,
    feather: Vec<MonaExtendedArtifact<'a>>,
    cup: Vec<MonaExtendedArtifact<'a>>,
    sand: Vec<MonaExtendedArtifact<'a>>,
    head: Vec<MonaExtendedArtifact<'a>>,
}

impl<'a> Serialize for MonaExtendedFormat<'a> {
     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_map(Some(6))?;
        root.serialize_entry("version", &self.version).unwrap();
        root.serialize_entry("flower", &self.flower).unwrap();
        root.serialize_entry("feather", &self.feather).unwrap();
        root.serialize_entry("sand", &self.sand).unwrap();
        root.serialize_entry("cup", &self.cup).unwrap();
        root.serialize_entry("head", &self.head).unwrap();
        root.end()
    }
}

impl<'a> MonaExtendedFormat<'a> {
    pub fn new(results: &'a [GenshinArtifact]) -> MonaExtendedFormat<'a> {
        let mut flower: Vec<MonaExtendedArtifact> = Vec::new();
        let mut feather: Vec<MonaExtendedArtifact> = Vec::new();
        let mut cup: Vec<MonaExtendedArtifact> = Vec::new();
        let mut sand: Vec<MonaExtendedArtifact> = Vec::new();
        let mut head: Vec<MonaExtendedArtifact> = Vec::new();

        for art in results.iter() {
            let wrapper = MonaExtendedArtifact(art);
            match art.slot {
                ArtifactSlot::Flower => flower.push(wrapper),
                ArtifactSlot::Feather => feather.push(wrapper),
                ArtifactSlot::Sand => sand.push(wrapper),
                ArtifactSlot::Goblet => cup.push(wrapper),
                ArtifactSlot::Head => head.push(wrapper),
            }
        }

        MonaExtendedFormat {
            flower,
            feather,
            cup,
            sand,
            head,
            version: String::from("1"),
        }
    }
}
