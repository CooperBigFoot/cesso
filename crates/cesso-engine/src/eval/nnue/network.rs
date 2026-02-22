//! NNUE network structure and forward pass.

use super::accumulator::Accumulator;

/// Hidden-layer dimension: 1024 neurons.
pub const HIDDEN: usize = 1024;

/// Number of output buckets (MaterialCount<8>).
pub const NUM_BUCKETS: usize = 8;

/// First-layer quantization factor.
const QA: i16 = 255;

/// Output-layer quantization factor.
const QB: i16 = 64;

/// Evaluation scale (maps to centipawns).
const SCALE: i32 = 400;

/// Quantized NNUE network loaded at compile time.
///
/// Binary layout (little-endian, `repr(C)`):
/// - `feature_weights`: 768 [`Accumulator`]s (768 * 1024 i16)
/// - `feature_bias`: 1 [`Accumulator`] (1024 i16)
/// - `output_weights`: NUM_BUCKETS * 2 * HIDDEN i16 (transposed, bucket-contiguous)
/// - `output_bias`: NUM_BUCKETS i16
#[repr(C)]
pub struct Network {
    /// Column-major `HIDDEN x 768` weight matrix. Quantization: QA.
    pub(crate) feature_weights: [Accumulator; 768],
    /// Bias vector of dimension HIDDEN. Quantization: QA.
    pub(crate) feature_bias: Accumulator,
    /// Row vectors `NUM_BUCKETS x (2 * HIDDEN)` output weights, bucket-contiguous. Quantization: QB.
    output_weights: [i16; NUM_BUCKETS * 2 * HIDDEN],
    /// Per-bucket scalar output bias. Quantization: QA * QB.
    output_bias: [i16; NUM_BUCKETS],
}

// SAFETY: Network is a plain-old-data type (repr(C)) with a known layout.
// The binary was written with the same layout by Bullet's quantized export.
// size_of::<Network>() == 1_607_744 (includes tail padding for align(64)).
static NNUE: Network = unsafe {
    std::mem::transmute(*include_bytes!("../../../../../nets/cesso-nnue-320.bin"))
};

impl Network {
    /// Return a reference to the statically-loaded NNUE network.
    #[inline]
    pub fn get() -> &'static Network {
        &NNUE
    }

    /// Forward pass: SCReLU activation, output dequantization.
    ///
    /// Returns centipawn evaluation from the `us` perspective.
    /// `bucket` selects the output head corresponding to the current material count.
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, bucket: usize) -> i32 {
        let mut output = 0i32;
        let base = bucket * 2 * HIDDEN;

        for (&x, &w) in us.vals.iter().zip(&self.output_weights[base..base + HIDDEN]) {
            output += screlu(x) * i32::from(w);
        }

        for (&x, &w) in them.vals.iter().zip(&self.output_weights[base + HIDDEN..base + 2 * HIDDEN]) {
            output += screlu(x) * i32::from(w);
        }

        // Dequantize: QA*QA*QB -> QA*QB
        output /= i32::from(QA);
        output += i32::from(self.output_bias[bucket]);
        output *= SCALE;
        // Final dequantization: remove QA*QB
        output /= i32::from(QA) * i32::from(QB);

        output
    }
}

/// SCReLU activation: clamp to [0, QA] then square.
#[inline]
fn screlu(x: i16) -> i32 {
    let y = i32::from(x).clamp(0, i32::from(QA));
    y * y
}
