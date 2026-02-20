//! Chess board files (columns a–h).

use std::fmt;

/// A file (column) on the chess board, from FileA to FileH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum File {
    FileA = 0,
    FileB = 1,
    FileC = 2,
    FileD = 3,
    FileE = 4,
    FileF = 5,
    FileG = 6,
    FileH = 7,
}

impl File {
    /// Total number of files.
    pub const COUNT: usize = 8;

    /// All files in index order.
    pub const ALL: [File; 8] = [
        File::FileA,
        File::FileB,
        File::FileC,
        File::FileD,
        File::FileE,
        File::FileF,
        File::FileG,
        File::FileH,
    ];

    /// Return the index (0..7).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Create a file from a zero-based index (0 = FileA, 7 = FileH).
    #[inline]
    pub const fn from_index(index: u8) -> Option<File> {
        match index {
            0 => Some(File::FileA),
            1 => Some(File::FileB),
            2 => Some(File::FileC),
            3 => Some(File::FileD),
            4 => Some(File::FileE),
            5 => Some(File::FileF),
            6 => Some(File::FileG),
            7 => Some(File::FileH),
            _ => None,
        }
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = (b'a' + self.index() as u8) as char;
        write!(f, "{c}")
    }
}

#[cfg(test)]
mod tests {
    use super::File;

    #[test]
    fn index_values() {
        assert_eq!(File::FileA.index(), 0);
        assert_eq!(File::FileH.index(), 7);
    }

    #[test]
    fn from_index_roundtrip() {
        for file in File::ALL {
            assert_eq!(File::from_index(file.index() as u8), Some(file));
        }
    }

    #[test]
    fn from_index_out_of_range() {
        assert_eq!(File::from_index(8), None);
        assert_eq!(File::from_index(255), None);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", File::FileA), "a");
        assert_eq!(format!("{}", File::FileH), "h");
    }

    #[test]
    fn ordering() {
        assert!(File::FileA < File::FileH);
        assert!(File::FileC < File::FileE);
    }

    #[test]
    fn all_and_count() {
        assert_eq!(File::COUNT, 8);
        assert_eq!(File::ALL.len(), File::COUNT);
    }
}
