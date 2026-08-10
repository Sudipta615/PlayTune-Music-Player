use db::models::{TrackAudioFeatures, TrackMoodScores};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub feature_idx: usize,
    pub threshold: f32,
    pub left_child: Option<usize>,
    pub right_child: Option<usize>,
    pub leaf_value: f32,
    pub is_leaf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub nodes: Vec<TreeNode>,
}

impl Tree {
    pub fn predict(&self, features: &[f32]) -> f32 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let mut curr = 0;
        while curr < self.nodes.len() {
            let node = &self.nodes[curr];
            if node.is_leaf {
                return node.leaf_value;
            }
            let val = features.get(node.feature_idx).copied().unwrap_or(0.0);
            if val <= node.threshold {
                if let Some(left) = node.left_child {
                    curr = left;
                } else {
                    return node.leaf_value;
                }
            } else if let Some(right) = node.right_child {
                curr = right;
            } else {
                return node.leaf_value;
            }
        }
        0.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoodEnsemble {
    pub trees: Vec<Tree>,
    pub base_score: f32,
}

impl MoodEnsemble {
    pub fn predict_probability(&self, features: &[f32]) -> f32 {
        if self.trees.is_empty() {
            return 0.5; // Neutral default probability if untrained
        }
        let mut raw_logit = self.base_score;
        for tree in &self.trees {
            raw_logit += tree.predict(features);
        }
        // Sigmoid mapping to [0.0, 1.0]
        1.0 / (1.0 + (-raw_logit).exp())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoodClassifierModel {
    pub happy: MoodEnsemble,
    pub sad: MoodEnsemble,
    pub calm: MoodEnsemble,
    pub energetic: MoodEnsemble,
    pub romantic: MoodEnsemble,
    pub party: MoodEnsemble,
    pub lofi: MoodEnsemble,
}

impl MoodClassifierModel {
    /// Load pre-trained model JSON definition.
    pub fn from_json(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse mood model JSON: {}", e))
    }

    /// Evaluate audio feature vector and calculate mood probabilities.
    pub fn classify(&self, features: &TrackAudioFeatures) -> TrackMoodScores {
        let vector = flatten_features(features);
        TrackMoodScores {
            track_id: features.track_id,
            happy: self.happy.predict_probability(&vector),
            sad: self.sad.predict_probability(&vector),
            calm: self.calm.predict_probability(&vector),
            energetic: self.energetic.predict_probability(&vector),
            romantic: self.romantic.predict_probability(&vector),
            party: self.party.predict_probability(&vector),
            lofi: self.lofi.predict_probability(&vector),
        }
    }
}

/// Convert TrackAudioFeatures struct into a flat float vector for model evaluation.
pub fn flatten_features(features: &TrackAudioFeatures) -> Vec<f32> {
    let mut vec = Vec::with_capacity(60);
    vec.push(features.tempo);
    vec.push(features.rms_mean);
    vec.push(features.rms_std);
    vec.push(features.zcr_mean);
    vec.push(features.zcr_std);
    vec.push(features.spectral_centroid_mean);
    vec.push(features.spectral_centroid_std);
    vec.push(features.spectral_rolloff_mean);
    vec.push(features.spectral_rolloff_std);
    vec.push(features.spectral_flatness_mean);
    vec.push(features.spectral_flatness_std);
    vec.push(features.spectral_flux_mean);
    vec.push(features.spectral_flux_std);
    vec.push(features.hpr);
    vec.push(features.spectral_contrast_mean);
    vec.push(features.spectral_contrast_std);
    vec.push(features.crest_factor);
    vec.push(features.mode_major_ratio);

    // MFCCs (13 pairs [mean, std])
    let mfccs: Vec<(f32, f32)> = serde_json::from_str(&features.mfcc_json).unwrap_or_default();
    for i in 0..13 {
        if let Some(&(m, s)) = mfccs.get(i) {
            vec.push(m);
            vec.push(s);
        } else {
            vec.push(0.0);
            vec.push(0.0);
        }
    }

    // Chroma (12 values)
    let chroma: Vec<f32> = serde_json::from_str(&features.chroma_json).unwrap_or_default();
    for i in 0..12 {
        if let Some(&c) = chroma.get(i) {
            vec.push(c);
        } else {
            vec.push(0.0);
        }
    }

    vec
}
