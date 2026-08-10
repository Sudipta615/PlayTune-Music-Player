#!/usr/bin/env python3
"""
PlayTune Mood Classifier Training Tool (v1.6.0)

Converts exported training_dataset.csv into pre-compiled LightGBM decision trees
formatted for PlayTune's pure-Rust GBDT model evaluator (assets/mood_models.json).
"""

import json
import os
import sys

try:
    import pandas as pd
    import numpy as np
    import lightgbm as lgb
except ImportError:
    print("LightGBM, pandas, and numpy are required for offline training.")
    print("Install via: pip install lightgbm pandas numpy")
    sys.exit(1)

try:
    from sklearn.model_selection import StratifiedKFold
    from sklearn.metrics import roc_auc_score, accuracy_score
    HAS_SKLEARN = True
except ImportError:
    HAS_SKLEARN = False

TARGET_MOODS = [
    "happy",
    "sad",
    "calm",
    "energetic",
    "romantic",
    "party",
    "lofi",
]

def convert_lgb_tree_to_dict(tree_dump):
    """Recursively convert LightGBM JSON tree dump into PlayTune TreeNode structure."""
    nodes = []
    
    def traverse(node):
        idx = len(nodes)
        nodes.append(None) # placeholder
        
        if "leaf_value" in node:
            nodes[idx] = {
                "feature_idx": 0,
                "threshold": 0.0,
                "left_child": None,
                "right_child": None,
                "leaf_value": float(node["leaf_value"]),
                "is_leaf": True
            }
        else:
            feat_name = node["split_feature"]
            feat_idx = int(feat_name) if isinstance(feat_name, int) or str(feat_name).isdigit() else 0
            thresh = float(node["threshold"])
            
            left_idx = traverse(node["left_child"])
            right_idx = traverse(node["right_child"])
            
            nodes[idx] = {
                "feature_idx": feat_idx,
                "threshold": thresh,
                "left_child": left_idx,
                "right_child": right_idx,
                "leaf_value": 0.0,
                "is_leaf": False
            }
        return idx

    traverse(tree_dump["tree_structure"])
    return {"nodes": nodes}

def train_mood_models(csv_path, output_json_path):
    if not os.path.exists(csv_path):
        print(f"Error: CSV dataset file '{csv_path}' not found.")
        print("Export it from PlayTune first using: playtune export-training-data")
        sys.exit(1)

    df = pd.read_csv(csv_path)
    print(f"Loaded training dataset with {len(df)} rows.")

    # Exclude metadata string columns and target labels
    non_feature_cols = ["song_id", "title", "artist", "album"] + TARGET_MOODS
    feature_cols = [c for c in df.columns if c not in non_feature_cols]
    print(f"Found {len(feature_cols)} numeric acoustic feature columns.")

    X = df[feature_cols].values.astype(np.float32)
    
    final_model = {}

    for mood in TARGET_MOODS:
        if mood not in df.columns:
            print(f"\nWarning: Mood '{mood}' not found in dataset. Skipping.")
            final_model[mood] = {"trees": [], "base_score": 0.0}
            continue
            
        y = df[mood].values.astype(np.int32)
        pos_count = np.sum(y == 1)
        neg_count = np.sum(y == 0)
        print(f"\nTraining LightGBM model for '{mood}' ({pos_count} positive, {neg_count} negative)...")

        if pos_count == 0:
            print(f"  No positive labels for '{mood}'. Skipping.")
            final_model[mood] = {"trees": [], "base_score": 0.0}
            continue

        # Regularized parameters tailored to small/medium datasets (~100-300 songs)
        params = {
            "objective": "binary",
            "metric": "binary_logloss",
            "boosting_type": "gbdt",
            "num_leaves": 8,
            "max_depth": 4,
            "learning_rate": 0.04,
            "min_child_samples": max(2, min(5, pos_count)),
            "feature_fraction": 0.75,
            "bagging_fraction": 0.8,
            "bagging_freq": 1,
            "reg_alpha": 0.1,
            "reg_lambda": 1.0,
            "verbosity": -1,
        }

        # Perform 5-Fold Stratified Cross Validation if scikit-learn is available
        if HAS_SKLEARN and pos_count >= 5 and neg_count >= 5:
            skf = StratifiedKFold(n_splits=min(5, pos_count), shuffle=True, random_state=42)
            cv_scores = []
            auc_scores = []
            for train_idx, val_idx in skf.split(X, y):
                X_tr, y_tr = X[train_idx], y[train_idx]
                X_val, y_val = X[val_idx], y[val_idx]

                d_tr = lgb.Dataset(X_tr, label=y_tr)
                d_val = lgb.Dataset(X_val, label=y_val, reference=d_tr)

                callbacks = [lgb.early_stopping(stopping_rounds=15, verbose=False)]
                gbm_cv = lgb.train(
                    params,
                    d_tr,
                    num_boost_round=100,
                    valid_sets=[d_val],
                    callbacks=callbacks
                )

                preds = gbm_cv.predict(X_val)
                pred_binary = (preds >= 0.5).astype(int)
                cv_scores.append(accuracy_score(y_val, pred_binary))
                try:
                    auc_scores.append(roc_auc_score(y_val, preds))
                except Exception:
                    pass

            if cv_scores:
                avg_acc = np.mean(cv_scores) * 100
                avg_auc = np.mean(auc_scores) if auc_scores else 0.0
                print(f"  5-Fold CV Accuracy: {avg_acc:.1f}% | ROC-AUC: {avg_auc:.3f}")

        # Final full dataset training
        train_data = lgb.Dataset(X, label=y, feature_name=[str(i) for i in range(X.shape[1])])
        gbm = lgb.train(params, train_data, num_boost_round=70)
        dump = gbm.dump_model()
        
        trees = []
        for tree_dump in dump["tree_info"]:
            converted = convert_lgb_tree_to_dict(tree_dump)
            trees.append(converted)

        base_score = 0.0

        final_model[mood] = {
            "trees": trees,
            "base_score": base_score
        }
        print(f"  Successfully trained {len(trees)} regularized trees for '{mood}'.")

    os.makedirs(os.path.dirname(os.path.abspath(output_json_path)), exist_ok=True)
    with open(output_json_path, "w") as f:
        json.dump(final_model, f, indent=2)

    print(f"\nModel training complete! Saved compiled model weights to '{output_json_path}'.")

if __name__ == "__main__":
    csv_file = sys.argv[1] if len(sys.argv) > 1 else "training_dataset.csv"
    output_file = sys.argv[2] if len(sys.argv) > 2 else "assets/mood_models.json"
    train_mood_models(csv_file, output_file)

