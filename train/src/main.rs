use bullet_lib::{
    game::{
        formats::sfbinpack::{
            TrainingDataEntry,
            chess::{r#move::MoveType, piecetype::PieceType},
        },
        inputs::Chess768,
        outputs::MaterialCount,
    },
    nn::optimiser::AdamW,
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::{ValueTrainerBuilder, loader::SfBinpackLoader},
};

// ── Architecture ────────────────────────────────────────────────────
// (768 -> HIDDEN)x2 -> NUM_BUCKETS, SCReLU activation, dual perspective
const HIDDEN: usize = 1024;
const NUM_BUCKETS: usize = 8;

// ── Quantization constants (must match inference in cesso) ──────────
const QA: i16 = 255;
const QB: i16 = 64;
const SCALE: i32 = 400;

// ── Training hyperparameters ────────────────────────────────────────
const SUPERBATCHES: usize = 320;
const BATCH_SIZE: usize = 16_384;
const BATCHES_PER_SUPERBATCH: usize = 6104;
const INITIAL_LR: f32 = 0.001;
const FINAL_LR: f32 = 0.000_003; // 0.001 * 0.3^5
const SAVE_RATE: usize = 20;

// ── Data ────────────────────────────────────────────────────────────
const DATA_PATHS: &[&str] = &[
    "data/test80-2024-01-jan-2tb7p.min-v2.v6.binpack",
    "data/test80-2024-02-feb-2tb7p.min-v2.v6.binpack",
    "data/test80-2024-03-mar-2tb7p.min-v2.v6.binpack",
    "data/test80-2024-04-apr-2tb7p.min-v2.v6.binpack",
    "data/test80-2024-05-may-2tb7p.min-v2.v6.binpack",
    "data/test80-2024-06-jun-2tb7p.min-v2.v6.binpack",
];
const DATA_BUFFER_MB: usize = 2048;
const DATA_THREADS: usize = 4;

fn main() {
    let mut trainer = ValueTrainerBuilder::default()
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(Chess768)
        .output_buckets(MaterialCount::<NUM_BUCKETS>)
        .save_format(&[
            SavedFormat::id("l0w").round().quantise::<i16>(QA),
            SavedFormat::id("l0b").round().quantise::<i16>(QA),
            SavedFormat::id("l1w").round().quantise::<i16>(QB).transpose(),
            SavedFormat::id("l1b").round().quantise::<i16>(QA * QB),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs, output_buckets| {
            let l0 = builder.new_affine("l0", 768, HIDDEN);
            let l1 = builder.new_affine("l1", 2 * HIDDEN, NUM_BUCKETS);

            let stm = l0.forward(stm_inputs).screlu();
            let ntm = l0.forward(ntm_inputs).screlu();
            l1.forward(stm.concat(ntm)).select(output_buckets)
        });

    let schedule = TrainingSchedule {
        net_id: "cesso-nnue".to_string(),
        eval_scale: SCALE as f32,
        steps: TrainingSteps {
            batch_size: BATCH_SIZE,
            batches_per_superbatch: BATCHES_PER_SUPERBATCH,
            start_superbatch: 1,
            end_superbatch: SUPERBATCHES,
        },
        wdl_scheduler: wdl::LinearWDL { start: 0.0, end: 0.5 },
        lr_scheduler: lr::CosineDecayLR {
            initial_lr: INITIAL_LR,
            final_lr: FINAL_LR,
            final_superbatch: SUPERBATCHES,
        },
        save_rate: SAVE_RATE,
    };

    let settings = LocalSettings {
        threads: DATA_THREADS,
        test_set: None,
        output_directory: "checkpoints",
        batch_queue_size: 64,
    };

    let data_loader =
        SfBinpackLoader::new_concat_multiple(DATA_PATHS, DATA_BUFFER_MB, DATA_THREADS, filter);

    trainer.run(&schedule, &settings, &data_loader);
}

/// Filter training positions for quality.
///
/// Keeps only quiet, non-trivial positions:
/// - Skip early game (ply < 16) to avoid opening book noise
/// - Skip positions where side to move is in check
/// - Skip extreme evaluations (|score| > 10000 cp)
/// - Skip positions where the recorded move is a capture or special move
fn filter(entry: &TrainingDataEntry) -> bool {
    entry.ply >= 16
        && !entry.pos.is_checked(entry.pos.side_to_move())
        && entry.score.unsigned_abs() <= 10000
        && entry.mv.mtype() == MoveType::Normal
        && entry.pos.piece_at(entry.mv.to()).piece_type() == PieceType::None
}
