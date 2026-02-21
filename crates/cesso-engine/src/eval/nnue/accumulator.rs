//! NNUE accumulator for incremental feature updates.

use cesso_core::{Board, Color, PieceKind};

use super::features::feature_index;
use super::network::{Network, HIDDEN};

/// Accumulated hidden-layer activations for one perspective.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Accumulator {
    pub(crate) vals: [i16; HIDDEN],
}

impl Accumulator {
    /// Full recompute: start from bias, then add all features on the board.
    pub fn refresh(board: &Board, perspective: Color, net: &Network) -> Self {
        let mut acc = net.feature_bias;

        for kind in PieceKind::ALL {
            for color in Color::ALL {
                let bb = board.pieces(kind) & board.side(color);
                for sq in bb {
                    let idx = feature_index(perspective, color, kind, sq);
                    acc.add_feature(idx, net);
                }
            }
        }

        acc
    }

    /// Incrementally add a feature (piece placed on a square).
    #[inline]
    pub fn add_feature(&mut self, idx: usize, net: &Network) {
        for (acc, &w) in self.vals.iter_mut().zip(&net.feature_weights[idx].vals) {
            *acc += w;
        }
    }

    /// Incrementally remove a feature (piece removed from a square).
    #[inline]
    pub fn remove_feature(&mut self, idx: usize, net: &Network) {
        for (acc, &w) in self.vals.iter_mut().zip(&net.feature_weights[idx].vals) {
            *acc -= w;
        }
    }
}
