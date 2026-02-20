//! Perft (performance test) for move generation correctness verification.

use crate::board::Board;
use crate::movegen::generate_legal_moves;

/// Count the number of leaf nodes at the given depth.
///
/// Depth 0 returns 1 (the current position). Depth 1 returns the number
/// of legal moves (bulk-counting optimization: no recursive make_move).
pub fn perft(board: &Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }

    let moves = generate_legal_moves(board);

    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0u64;
    for mv in moves.as_slice() {
        let child = board.make_move(*mv);
        nodes += perft(&child, depth - 1);
    }
    nodes
}

/// Run perft with per-move breakdown (useful for debugging).
///
/// Returns a vector of `(uci_move, node_count)` pairs sorted alphabetically.
pub fn divide(board: &Board, depth: usize) -> Vec<(String, u64)> {
    let moves = generate_legal_moves(board);
    let mut results: Vec<(String, u64)> = moves
        .as_slice()
        .iter()
        .map(|mv| {
            let child = board.make_move(*mv);
            let count = if depth <= 1 { 1 } else { perft(&child, depth - 1) };
            (mv.to_uci(), count)
        })
        .collect();
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    // --- Position 1: Starting position ---
    // rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1

    #[test]
    fn perft_startpos_depth_1() {
        let board = Board::starting_position();
        assert_eq!(perft(&board, 1), 20);
    }

    #[test]
    fn perft_startpos_depth_2() {
        let board = Board::starting_position();
        assert_eq!(perft(&board, 2), 400);
    }

    #[test]
    fn perft_startpos_depth_3() {
        let board = Board::starting_position();
        assert_eq!(perft(&board, 3), 8_902);
    }

    #[test]
    fn perft_startpos_depth_4() {
        let board = Board::starting_position();
        assert_eq!(perft(&board, 4), 197_281);
    }

    #[test]
    #[ignore] // slow
    fn perft_startpos_depth_5() {
        let board = Board::starting_position();
        assert_eq!(perft(&board, 5), 4_865_609);
    }

    // --- Position 2: Kiwipete ---
    // r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1

    fn kiwipete() -> Board {
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
            .parse()
            .unwrap()
    }

    #[test]
    fn perft_kiwipete_depth_1() {
        assert_eq!(perft(&kiwipete(), 1), 48);
    }

    #[test]
    fn perft_kiwipete_depth_2() {
        assert_eq!(perft(&kiwipete(), 2), 2_039);
    }

    #[test]
    fn perft_kiwipete_depth_3() {
        assert_eq!(perft(&kiwipete(), 3), 97_862);
    }

    #[test]
    fn perft_kiwipete_depth_4() {
        assert_eq!(perft(&kiwipete(), 4), 4_085_603);
    }

    #[test]
    #[ignore] // slow
    fn perft_kiwipete_depth_5() {
        assert_eq!(perft(&kiwipete(), 5), 193_690_690);
    }

    // --- Position 3 ---
    // 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1

    fn position3() -> Board {
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"
            .parse()
            .unwrap()
    }

    #[test]
    fn perft_pos3_depth_1() {
        assert_eq!(perft(&position3(), 1), 14);
    }

    #[test]
    fn perft_pos3_depth_2() {
        assert_eq!(perft(&position3(), 2), 191);
    }

    #[test]
    fn perft_pos3_depth_3() {
        assert_eq!(perft(&position3(), 3), 2_812);
    }

    #[test]
    fn perft_pos3_depth_4() {
        assert_eq!(perft(&position3(), 4), 43_238);
    }

    #[test]
    #[ignore] // slow
    fn perft_pos3_depth_5() {
        assert_eq!(perft(&position3(), 5), 674_624);
    }

    // --- Position 4 ---
    // r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1

    fn position4() -> Board {
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"
            .parse()
            .unwrap()
    }

    #[test]
    fn perft_pos4_depth_1() {
        assert_eq!(perft(&position4(), 1), 6);
    }

    #[test]
    fn perft_pos4_depth_2() {
        assert_eq!(perft(&position4(), 2), 264);
    }

    #[test]
    fn perft_pos4_depth_3() {
        assert_eq!(perft(&position4(), 3), 9_467);
    }

    #[test]
    fn perft_pos4_depth_4() {
        assert_eq!(perft(&position4(), 4), 422_333);
    }

    #[test]
    #[ignore] // slow
    fn perft_pos4_depth_5() {
        assert_eq!(perft(&position4(), 5), 15_833_292);
    }

    // --- Position 5 ---
    // rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8

    fn position5() -> Board {
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
            .parse()
            .unwrap()
    }

    #[test]
    fn perft_pos5_depth_1() {
        assert_eq!(perft(&position5(), 1), 44);
    }

    #[test]
    fn perft_pos5_depth_2() {
        assert_eq!(perft(&position5(), 2), 1_486);
    }

    #[test]
    fn perft_pos5_depth_3() {
        assert_eq!(perft(&position5(), 3), 62_379);
    }

    #[test]
    fn perft_pos5_depth_4() {
        assert_eq!(perft(&position5(), 4), 2_103_487);
    }

    #[test]
    #[ignore] // slow
    fn perft_pos5_depth_5() {
        assert_eq!(perft(&position5(), 5), 89_941_194);
    }

    // --- divide test ---

    #[test]
    fn divide_startpos_depth_1() {
        let board = Board::starting_position();
        let results = divide(&board, 1);
        assert_eq!(results.len(), 20);
        // Each move at depth 1 should have count 1
        for (_, count) in &results {
            assert_eq!(*count, 1);
        }
    }

    // --- depth 0 ---

    #[test]
    fn perft_depth_0() {
        let board = Board::starting_position();
        assert_eq!(perft(&board, 0), 1);
    }
}
