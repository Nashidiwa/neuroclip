//! Neuro Swarm Cultural Emote Dictionary & Weight Matrix.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/PRD.md` §6.2, `docs/adr/ADR-004`, and `docs/TASK_BOARD.md` §TASK-M4-10.

use std::collections::HashMap;
use std::fs::File;
use std::hash::{BuildHasher, Hasher};
use std::io::BufReader;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, HighlightError, StorageError};

/// Ultra-fast non-cryptographic FNV-1a hasher for high-throughput in-memory token lookup (AC-10.1, ED-10.1).
#[derive(Debug, Clone, Copy, Default)]
pub struct FastHasher(u64);

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(1_099_511_628_211);
        }
    }
}

/// Hasher builder producing `FastHasher` seeded with standard FNV offset basis.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildFastHasher;

impl BuildHasher for BuildFastHasher {
    type Hasher = FastHasher;

    #[inline]
    fn build_hasher(&self) -> FastHasher {
        FastHasher(14_695_981_039_346_656_037)
    }
}

/// Cultural Category of Neuro Swarm Emotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmEmoteCategory {
    /// Laughter and humor reactions (KEKW, LUL, OMEGALUL, 💀, LMAO, LOL).
    Humor,
    /// Shock, surprise, and cringe reactions (Pog, PogBones, Aware, Clueless, WTF).
    Surprise,
    /// Special community lore, characters, and recurring memes (Tutel, SOOP, Erm, Heart, Vedal, Evil, Neuro).
    SwarmLore,
}

/// Metadata definition of an emote in the Swarm Dictionary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmEmoteDefinition {
    /// Canonical display identifier (e.g. "KEKW", "Tutel", "💀", "SOOP").
    pub canonical_name: String,
    /// Semantic category classification.
    pub category: SwarmEmoteCategory,
    /// Quantitative cultural weight bonus (e.g. 1.0 for general laugh, 1.8 - 2.0 for core Swarm lore).
    pub weight: f64,
    /// Case-insensitive aliases or alternate spellings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

impl SwarmEmoteDefinition {
    /// Create a new emote definition.
    #[must_use]
    pub fn new(
        canonical_name: impl Into<String>,
        category: SwarmEmoteCategory,
        weight: f64,
        aliases: &[&str],
    ) -> Self {
        Self {
            canonical_name: canonical_name.into(),
            category,
            weight,
            aliases: aliases.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

/// In-memory Swarm Cultural Dictionary with $O(1)$ case-insensitive lookup.
#[derive(Debug, Clone, Serialize)]
pub struct SwarmDictionary {
    /// Schema or dictionary version.
    pub version: String,
    /// List of canonical emote definitions.
    pub emotes: Vec<SwarmEmoteDefinition>,
    /// Internal fast lowercase index map (zero heap allocations on ASCII lookups <= 64 bytes).
    #[serde(skip)]
    lookup_map: HashMap<String, usize, BuildFastHasher>,
}

impl<'de> Deserialize<'de> for SwarmDictionary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawDictionary {
            #[serde(default = "default_version_str")]
            version: String,
            emotes: Vec<SwarmEmoteDefinition>,
        }

        fn default_version_str() -> String {
            "1.0.0".to_string()
        }

        let raw = RawDictionary::deserialize(deserializer)?;
        let mut dict = Self::new(raw.emotes);
        dict.version = raw.version;
        Ok(dict)
    }
}

impl Default for SwarmDictionary {
    fn default() -> Self {
        Self::build_default_dictionary()
    }
}

impl SwarmDictionary {
    /// Construct a dictionary from a vector of emote definitions, building the lowercase index map.
    #[must_use]
    pub fn new(emotes: Vec<SwarmEmoteDefinition>) -> Self {
        let mut lookup_map = HashMap::with_capacity_and_hasher(emotes.len() * 4, BuildFastHasher);
        for (idx, def) in emotes.iter().enumerate() {
            lookup_map.insert(def.canonical_name.to_lowercase(), idx);
            for alias in &def.aliases {
                lookup_map.insert(alias.to_lowercase(), idx);
            }
        }
        Self {
            version: "1.0.0".to_string(),
            emotes,
            lookup_map,
        }
    }

    /// Build the standard Neuro-sama community cultural dictionary with canonical weights.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn build_default_dictionary() -> Self {
        let emotes = vec![
            // -------------------------------------------------------------------
            // Swarm Lore & Character Identity (AC-10.2, PRD §6.2)
            // -------------------------------------------------------------------
            SwarmEmoteDefinition::new(
                "Tutel",
                SwarmEmoteCategory::SwarmLore,
                2.0,
                &["tutel", "turtle", "vedaltutel"],
            ),
            SwarmEmoteDefinition::new(
                "SOOP",
                SwarmEmoteCategory::SwarmLore,
                2.0,
                &["soop", "soup"],
            ),
            SwarmEmoteDefinition::new("Erm", SwarmEmoteCategory::SwarmLore, 2.0, &["erm", "ermm"]),
            SwarmEmoteDefinition::new(
                "Heart",
                SwarmEmoteCategory::SwarmLore,
                2.0,
                &["heart", "<3", "neuroheart", "evilheart"],
            ),
            SwarmEmoteDefinition::new("Gymbag", SwarmEmoteCategory::SwarmLore, 1.5, &["gymbag"]),
            SwarmEmoteDefinition::new("Dinking", SwarmEmoteCategory::SwarmLore, 1.5, &["dinking"]),
            SwarmEmoteDefinition::new(
                "Filtered",
                SwarmEmoteCategory::SwarmLore,
                1.5,
                &["filtered", "filter", "leaking"],
            ),
            SwarmEmoteDefinition::new(
                "Abandoned",
                SwarmEmoteCategory::SwarmLore,
                1.5,
                &["abandoned"],
            ),
            SwarmEmoteDefinition::new(
                "Vedal",
                SwarmEmoteCategory::SwarmLore,
                1.5,
                &["vedal", "vedal987", "vedalpls"],
            ),
            SwarmEmoteDefinition::new(
                "Evil",
                SwarmEmoteCategory::SwarmLore,
                1.5,
                &["evil", "evilneuro"],
            ),
            SwarmEmoteDefinition::new(
                "Neuro",
                SwarmEmoteCategory::SwarmLore,
                1.5,
                &["neuro", "neurosama", "neurojam"],
            ),
            SwarmEmoteDefinition::new("Cooked", SwarmEmoteCategory::SwarmLore, 1.4, &["cooked"]),
            SwarmEmoteDefinition::new(
                "Unhinged",
                SwarmEmoteCategory::SwarmLore,
                1.4,
                &["unhinged"],
            ),
            SwarmEmoteDefinition::new("SoCute", SwarmEmoteCategory::SwarmLore, 1.4, &["socute"]),
            SwarmEmoteDefinition::new("AYAYA", SwarmEmoteCategory::SwarmLore, 1.2, &["ayaya"]),
            SwarmEmoteDefinition::new("Anny", SwarmEmoteCategory::SwarmLore, 1.4, &["anny"]),
            SwarmEmoteDefinition::new("Camila", SwarmEmoteCategory::SwarmLore, 1.4, &["camila"]),
            SwarmEmoteDefinition::new("Bedge", SwarmEmoteCategory::SwarmLore, 1.2, &["bedge"]),
            SwarmEmoteDefinition::new("Corpa", SwarmEmoteCategory::SwarmLore, 1.2, &["corpa"]),
            SwarmEmoteDefinition::new(
                "GIGACHAD",
                SwarmEmoteCategory::SwarmLore,
                1.4,
                &["gigachad"],
            ),
            SwarmEmoteDefinition::new("pepeJAM", SwarmEmoteCategory::SwarmLore, 1.3, &["pepejam"]),
            SwarmEmoteDefinition::new("CatJAM", SwarmEmoteCategory::SwarmLore, 1.3, &["catjam"]),
            SwarmEmoteDefinition::new(
                "SOUTHSIDE",
                SwarmEmoteCategory::SwarmLore,
                1.3,
                &["southside"],
            ),
            // -------------------------------------------------------------------
            // Humor & Laughter Reactions (AC-10.2, UT-10.1)
            // -------------------------------------------------------------------
            SwarmEmoteDefinition::new("KEKW", SwarmEmoteCategory::Humor, 1.5, &["kekw", "kek"]),
            SwarmEmoteDefinition::new(
                "LUL",
                SwarmEmoteCategory::Humor,
                1.5,
                &["lul", "lulw", "classiclul", "classic lul"],
            ),
            SwarmEmoteDefinition::new("OMEGALUL", SwarmEmoteCategory::Humor, 1.5, &["omegalul"]),
            SwarmEmoteDefinition::new("💀", SwarmEmoteCategory::Humor, 1.5, &["💀"]),
            SwarmEmoteDefinition::new("ICANT", SwarmEmoteCategory::Humor, 1.2, &["icant"]),
            SwarmEmoteDefinition::new("LOLW", SwarmEmoteCategory::Humor, 1.1, &["lolw"]),
            SwarmEmoteDefinition::new("LMAO", SwarmEmoteCategory::Humor, 1.0, &["lmao"]),
            SwarmEmoteDefinition::new("LOL", SwarmEmoteCategory::Humor, 1.0, &["lol"]),
            SwarmEmoteDefinition::new("XD", SwarmEmoteCategory::Humor, 1.0, &["xd"]),
            // -------------------------------------------------------------------
            // Surprise, Shock & Cringe Reactions (AC-10.2, ADR-004)
            // -------------------------------------------------------------------
            SwarmEmoteDefinition::new("Aware", SwarmEmoteCategory::Surprise, 1.5, &["aware"]),
            SwarmEmoteDefinition::new("Clueless", SwarmEmoteCategory::Surprise, 1.5, &["clueless"]),
            SwarmEmoteDefinition::new(
                "Pog",
                SwarmEmoteCategory::Surprise,
                1.2,
                &["pog", "pogchamp"],
            ),
            SwarmEmoteDefinition::new("PogBones", SwarmEmoteCategory::Surprise, 1.3, &["pogbones"]),
            SwarmEmoteDefinition::new("WTF", SwarmEmoteCategory::Surprise, 1.2, &["wtf"]),
            SwarmEmoteDefinition::new("monkaW", SwarmEmoteCategory::Surprise, 1.3, &["monkaw"]),
            SwarmEmoteDefinition::new("DESPAIR", SwarmEmoteCategory::Surprise, 1.3, &["despair"]),
            SwarmEmoteDefinition::new("HUH", SwarmEmoteCategory::Surprise, 1.2, &["huh"]),
            SwarmEmoteDefinition::new("monkaS", SwarmEmoteCategory::Surprise, 1.2, &["monkas"]),
            SwarmEmoteDefinition::new("WutFace", SwarmEmoteCategory::Surprise, 1.1, &["wutface"]),
        ];

        Self::new(emotes)
    }

    /// Fast $O(1)$ case-insensitive lookup.
    ///
    /// For ASCII tokens $\le 64$ bytes, uses an internal stack buffer and FNV-1a hasher
    /// to guarantee zero heap allocation and minimal CPU cycles on the hot path (AC-10.1, ED-10.1).
    #[inline]
    #[must_use]
    pub fn lookup(&self, token: &str) -> Option<&SwarmEmoteDefinition> {
        if token.is_empty() {
            return None;
        }

        // Direct check for Unicode skull emoji
        if token == "💀" {
            return self
                .lookup_map
                .get("💀")
                .and_then(|&idx| self.emotes.get(idx));
        }

        // Direct check for heart emoticon <3
        if token == "<3" {
            return self
                .lookup_map
                .get("<3")
                .and_then(|&idx| self.emotes.get(idx));
        }

        // Zero-allocation stack buffer for ASCII strings <= 64 bytes
        if token.is_ascii() && token.len() <= 64 {
            let mut buf = [0u8; 64];
            let bytes = token.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                buf[i] = b.to_ascii_lowercase();
            }
            if let Ok(lower_slice) = std::str::from_utf8(&buf[..bytes.len()]) {
                if let Some(&idx) = self.lookup_map.get(lower_slice) {
                    return self.emotes.get(idx);
                }
            }
            return None;
        }

        let lower = token.to_lowercase();
        self.lookup_map
            .get(&lower)
            .and_then(|&idx| self.emotes.get(idx))
    }

    /// Check if a word token belongs to the Swarm dictionary (UT-10.2).
    #[must_use]
    pub fn contains(&self, token: &str) -> bool {
        self.lookup(token).is_some()
    }

    /// Total number of canonical emote definitions in the dictionary.
    #[must_use]
    pub fn len(&self) -> usize {
        self.emotes.len()
    }

    /// Check if dictionary contains zero emotes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.emotes.is_empty()
    }

    /// Load dictionary from a JSON file on disk, with automatic fallback to default.
    #[must_use]
    pub fn from_file_or_default<P: AsRef<Path>>(path: Option<P>) -> Self {
        if let Some(p) = path {
            if let Ok(dict) = Self::from_file(p) {
                return dict;
            }
        }
        Self::default()
    }

    /// Load dictionary from a specific JSON file.
    ///
    /// # Errors
    /// Returns `AppError::Storage` or `AppError::Highlight` if the file cannot be read or parsed.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, AppError> {
        let p = path.as_ref();
        if !p.exists() {
            return Err(AppError::Storage(StorageError::FileNotFound(
                p.display().to_string(),
            )));
        }

        let file = File::open(p).map_err(|e| {
            AppError::Storage(StorageError::DiskCheckFailed {
                path: p.display().to_string(),
                message: e.to_string(),
            })
        })?;

        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(|e| {
            AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                "Failed to parse swarm dictionary JSON: {e}"
            )))
        })
    }
}

/// Convenience function returning the default Swarm dictionary.
#[must_use]
pub fn default_swarm_dictionary() -> SwarmDictionary {
    SwarmDictionary::default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub mod tests {
    use super::*;

    #[test]
    fn test_default_dictionary_loaded() {
        let dict = default_swarm_dictionary();
        assert!(
            dict.len() >= 30,
            "Must have comprehensive emote definitions"
        );
        assert_eq!(dict.version, "1.0.0");
    }

    #[test]
    fn test_ut_10_2_case_insensitivity_and_unicode() {
        let dict = default_swarm_dictionary();

        // Case insensitivity checks
        assert!(dict.contains("kekw"));
        assert!(dict.contains("KEKW"));
        assert!(dict.contains("Kekw"));
        assert!(dict.contains("TUTEL"));
        assert!(dict.contains("tutel"));
        assert!(dict.contains("Tutel"));
        assert!(dict.contains("ERM"));
        assert!(dict.contains("erm"));
        assert!(dict.contains("Erm"));
        assert!(dict.contains("SOOP"));
        assert!(dict.contains("soop"));
        assert!(dict.contains("PogBones"));
        assert!(dict.contains("pogbones"));

        // Unicode and special symbol checks
        assert!(dict.contains("💀"));
        assert!(dict.contains("<3"));

        // Negative non-matches (prevent false positives)
        assert!(!dict.contains("medieval"));
        assert!(!dict.contains("talking"));
        assert!(!dict.contains("pogo"));
        assert!(!dict.contains("neurological"));
        assert!(!dict.contains("hello"));
        assert!(!dict.contains("world"));
    }

    #[test]
    fn test_cultural_weights_and_categories() {
        let dict = default_swarm_dictionary();

        let tutel = dict.lookup("Tutel").expect("Tutel must exist");
        assert_eq!(tutel.canonical_name, "Tutel");
        assert_eq!(tutel.category, SwarmEmoteCategory::SwarmLore);
        assert!((tutel.weight - 2.0).abs() < 1e-6);

        let soop = dict.lookup("SOOP").expect("SOOP must exist");
        assert_eq!(soop.category, SwarmEmoteCategory::SwarmLore);
        assert!((soop.weight - 2.0).abs() < 1e-6);

        let erm = dict.lookup("Erm").expect("Erm must exist");
        assert_eq!(erm.category, SwarmEmoteCategory::SwarmLore);
        assert!((erm.weight - 2.0).abs() < 1e-6);

        let heart = dict.lookup("<3").expect("<3 must exist");
        assert_eq!(heart.canonical_name, "Heart");
        assert_eq!(heart.category, SwarmEmoteCategory::SwarmLore);
        assert!((heart.weight - 2.0).abs() < 1e-6);

        let kekw = dict.lookup("KEKW").expect("KEKW must exist");
        assert_eq!(kekw.category, SwarmEmoteCategory::Humor);
        assert!((kekw.weight - 1.5).abs() < 1e-6);

        let skull = dict.lookup("💀").expect("Skull must exist");
        assert_eq!(skull.category, SwarmEmoteCategory::Humor);
        assert!((skull.weight - 1.5).abs() < 1e-6);

        let aware = dict.lookup("Aware").expect("Aware must exist");
        assert_eq!(aware.category, SwarmEmoteCategory::Surprise);
        assert!((aware.weight - 1.5).abs() < 1e-6);

        let pog = dict.lookup("pog").expect("Pog must exist");
        assert_eq!(pog.category, SwarmEmoteCategory::Surprise);
        assert!((pog.weight - 1.2).abs() < 1e-6);
    }

    #[test]
    fn test_custom_dictionary_serialization_roundtrip() {
        let dict = default_swarm_dictionary();
        let serialized = serde_json::to_string(&dict).expect("Must serialize");
        let deserialized: SwarmDictionary =
            serde_json::from_str(&serialized).expect("Must deserialize");

        assert_eq!(deserialized.len(), dict.len());
        assert!(deserialized.contains("Tutel"));
        assert!(deserialized.contains("tutel"));
        assert!(deserialized.contains("💀"));
        assert!(deserialized.contains("<3"));
    }
}
