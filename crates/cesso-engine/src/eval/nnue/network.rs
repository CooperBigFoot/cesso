//! NNUE network structure and forward pass.

use super::accumulator::Accumulator;

/// Hidden-layer dimension: 128 neurons.
pub const HIDDEN: usize = 128;

/// First-layer quantization factor.
const QA: i16 = 255;

/// Output-layer quantization factor.
const QB: i16 = 64;

/// Evaluation scale (maps to centipawns).
const SCALE: i32 = 400;

/// Quantized NNUE network loaded at compile time.
///
/// Binary layout (little-endian, `repr(C)`):
/// - `feature_weights`: 768 [`Accumulator`]s (768 * 128 i16)
/// - `feature_bias`: 1 [`Accumulator`] (128 i16)
/// - `output_weights`: 2 * HIDDEN i16
/// - `output_bias`: 1 i16
#[repr(C)]
pub struct Network {
    /// Column-major `HIDDEN x 768` weight matrix. Quantization: QA.
    pub(crate) feature_weights: [Accumulator; 768],
    /// Bias vector of dimension HIDDEN. Quantization: QA.
    pub(crate) feature_bias: Accumulator,
    /// Row vector `1 x (2 * HIDDEN)` output weights. Quantization: QB.
    output_weights: [i16; 2 * HIDDEN],
    /// Scalar output bias. Quantization: QA * QB.
    output_bias: i16,
}

// SAFETY: Network is a plain-old-data type (repr(C)) with a known layout.
// The binary was written with the same layout by Bullet's quantized export.
// size_of::<Network>() == 197_440 (includes 62 bytes of tail padding for align(64)).
static NNUE: Network = unsafe {
    std::mem::transmute(*include_bytes!("../../../../../nets/cesso-nnue-40.bin"))
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
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator) -> i32 {
        let mut output = 0i32;

        for (&x, &w) in us.vals.iter().zip(&self.output_weights[..HIDDEN]) {
            output += screlu(x) * i32::from(w);
        }

        for (&x, &w) in them.vals.iter().zip(&self.output_weights[HIDDEN..]) {
            output += screlu(x) * i32::from(w);
        }

        // Dequantize: QA*QA*QB -> QA*QB
        output /= i32::from(QA);
        output += i32::from(self.output_bias);
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
