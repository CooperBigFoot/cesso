use bullet_lib::{
    game::{
        formats::sfbinpack::{
            TrainingDataEntry,
            chess::{r#move::MoveType, piecetype::PieceType},
        },
        inputs::Chess768,
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
// (768 -> HIDDEN)x2 -> 1, SCReLU activation, dual perspective
const HIDDEN: usize = 128;

// ── Quantization constants (must match inference in cesso) ──────────
const QA: i16 = 255;
const QB: i16 = 64;
const SCALE: i32 = 400;

// ── Training hyperparameters ────────────────────────────────────────
const SUPERBATCHES: usize = 40;
const BATCH_SIZE: usize = 16_384;
const BATCHES_PER_SUPERBATCH: usize = 6104;
const INITIAL_LR: f32 = 0.001;
const FINAL_LR: f32 = 0.000_003; // 0.001 * 0.3^5
const WDL: f32 = 0.75;
const SAVE_RATE: usize = 10;

// ── Data ────────────────────────────────────────────────────────────
const DATA_PATH: &str = "data/test80.binpack";
const DATA_BUFFER_MB: usize = 1024;
const DATA_THREADS: usize = 4;

fn main() {
    let mut trainer = ValueTrainerBuilder::default()
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(Chess768)
        .save_format(&[
            SavedFormat::id("l0w").round().quantise::<i16>(QA),
            SavedFormat::id("l0b").round().quantise::<i16>(QA),
            SavedFormat::id("l1w").round().quantise::<i16>(QB),
            SavedFormat::id("l1b").round().quantise::<i16>(QA * QB),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs| {
            let l0 = builder.new_affine("l0", 768, HIDDEN);
            let l1 = builder.new_affine("l1", 2 * HIDDEN, 1);

            let stm = l0.forward(stm_inputs).screlu();
            let ntm = l0.forward(ntm_inputs).screlu();
            l1.forward(stm.concat(ntm))
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
        wdl_scheduler: wdl::ConstantWDL { value: WDL },
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

    let data_loader = SfBinpackLoader::new(DATA_PATH, DATA_BUFFER_MB, DATA_THREADS, filter);

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

// ────────────────────────────────────────────────────────────────────
// Everything below is the inference code for reference.
// Copy this into cesso-engine when implementing NNUE evaluation.
// ────────────────────────────────────────────────────────────────────

/*
/// Quantised network loaded at compile time.
/// In cesso, use: `include_bytes!("path/to/cesso-nnue.bin")`
static NNUE: Network =
    unsafe { std::mem::transmute(*include_bytes!("../checkpoints/cesso-nnue-40/cesso-nnue-40.bin")) };

const HIDDEN: usize = 128;
const QA: i16 = 255;
const QB: i16 = 64;
const SCALE: i32 = 400;

#[inline]
fn screlu(x: i16) -> i32 {
    let y = i32::from(x).clamp(0, i32::from(QA));
    y * y
}

#[repr(C)]
pub struct Network {
    /// Column-major `HIDDEN x 768` weight matrix. Quantization: QA.
    feature_weights: [Accumulator; 768],
    /// Bias vector of dimension HIDDEN. Quantization: QA.
    feature_bias: Accumulator,
    /// Row vector `1 x (2 * HIDDEN)` output weights. Quantization: QB.
    output_weights: [i16; 2 * HIDDEN],
    /// Scalar output bias. Quantization: QA * QB.
    output_bias: i16,
}

impl Network {
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator) -> i32 {
        let mut output = 0i32;

        for (&x, &w) in us.vals.iter().zip(&self.output_weights[..HIDDEN]) {
            output += screlu(x) * i32::from(w);
        }

        for (&x, &w) in them.vals.iter().zip(&self.output_weights[HIDDEN..]) {
            output += screlu(x) * i32::from(w);
        }

        // QA*QA*QB -> QA*QB
        output /= i32::from(QA);
        output += i32::from(self.output_bias);
        output *= SCALE;
        // Remove quantization entirely
        output /= i32::from(QA) * i32::from(QB);

        output
    }
}

#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN],
}

impl Accumulator {
    pub fn new(net: &Network) -> Self {
        net.feature_bias
    }

    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        for (acc, &w) in self.vals.iter_mut().zip(&net.feature_weights[feature_idx].vals) {
            *acc += w;
        }
    }

    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        for (acc, &w) in self.vals.iter_mut().zip(&net.feature_weights[feature_idx].vals) {
            *acc -= w;
        }
    }
}
*/
