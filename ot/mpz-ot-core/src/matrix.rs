use std::{
    ops::Range,
    slice::{Chunks, ChunksMut},
};

use itybity::IntoBits;
use matrix_transpose::transpose_bits;
use thiserror::Error;
use utils::bits::FromBits;

/// Byte matrix.
#[derive(Default, Debug, PartialEq, Clone)]
pub(crate) struct Matrix {
    data: Vec<u8>,
    row_width: usize,
}

impl Matrix {
    /// Creates a new matrix
    pub(crate) fn new(data: Vec<u8>, row_width: usize) -> Self {
        Self { data, row_width }
    }

    /// Sets the row length
    ///
    /// # Panics
    ///
    /// Panics if the resulting matrix is not rectangular
    pub(crate) fn set_row_width(&mut self, len: usize) {
        assert_eq!(self.data.len() % len, 0, "matrix is not rectangular");
        self.row_width = len;
    }

    /// Returns the number of elements
    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns the number of rows
    pub(crate) fn rows(&self) -> usize {
        self.data.len() / self.row_width
    }

    /// Returns a reference to the data
    pub(crate) fn inner(&self) -> &[u8] {
        &self.data
    }

    /// Returns the data
    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.data
    }

    /// Pushes a row onto the end of the matrix
    ///
    /// # Panics
    ///
    /// Panics if the width of the row is not equal to the row width
    pub(crate) fn push_row(&mut self, row: &[u8]) {
        assert_eq!(row.len(), self.row_width, "row width does not match");
        self.data.extend(row);
    }

    /// Extends the matrix with the given matrix
    ///
    /// # Panics
    ///
    /// Panics if the row width does not match
    pub(crate) fn extend(&mut self, matrix: Matrix) {
        assert_eq!(matrix.row_width, self.row_width, "row width does not match");
        self.data.extend(matrix.data);
    }

    /// Takes all rows from the matrix
    pub(crate) fn take(&mut self) -> Matrix {
        Matrix {
            data: std::mem::take(&mut self.data),
            row_width: self.row_width,
        }
    }

    /// Transpose bitwise
    ///
    /// The matrix is treated as a matrix of bits, then transposed and again encoded as a byte
    /// matrix
    pub(crate) fn transpose_bits(&mut self) -> Result<(), MatrixError> {
        let prev_row_count = self.rows();

        #[allow(clippy::all)]
        fn transpose(data: &[u8], row_width: usize) -> Vec<u8> {
            use itybity::*;

            let bits: Vec<Vec<bool>> = data.chunks(row_width).map(|x| x.to_lsb0_vec()).collect();
            let col_count = bits[0].len();
            let row_count = bits.len();

            let mut bits_: Vec<Vec<bool>> = vec![vec![false; row_count]; col_count];

            for j in 0..row_count {
                for i in 0..col_count {
                    bits_[i][j] = bits[j][i];
                }
            }

            bits_.into_iter().flat_map(Vec::<u8>::from_lsb0).collect()
        }

        self.data = transpose(&self.data, self.row_width);

        self.row_width = prev_row_count / 8;

        Ok(())
    }

    /// Splits the matrix at the given row index.
    ///
    /// Returns the split off rows.
    pub(crate) fn split_off_rows(&mut self, idx: usize) -> Self {
        Self {
            data: self.data.split_off(idx * self.row_width),
            row_width: self.row_width,
        }
    }

    /// Drains the provided rows from the matrix
    ///
    /// Returns the drained rows
    pub(crate) fn drain_rows(&mut self, range: Range<usize>) -> Self {
        Self {
            data: self
                .data
                .drain(range.start * self.row_width..range.end * self.row_width)
                .collect(),
            row_width: self.row_width,
        }
    }

    /// Truncate the matrix to the given number of rows
    pub(crate) fn truncate_rows(&mut self, len: usize) {
        self.data.truncate(len * self.row_width);
    }

    /// Iterator over rows
    pub(crate) fn iter_rows(&self) -> Chunks<u8> {
        self.data.chunks(self.row_width)
    }

    /// Parallel iterator over rows
    #[cfg(feature = "rayon")]
    pub(crate) fn par_iter_rows(&self) -> rayon::slice::Chunks<u8> {
        use rayon::slice::ParallelSlice;

        self.data.par_chunks(self.row_width)
    }

    /// Mutable iterator over rows
    pub(crate) fn iter_rows_mut(&mut self) -> ChunksMut<u8> {
        let row_width = self.row_width;
        self.data.chunks_mut(self.row_width)
    }

    /// Parallel mutable iterator over rows
    #[cfg(feature = "rayon")]
    pub(crate) fn par_iter_rows_mut(&mut self) -> rayon::slice::ChunksMut<u8> {
        use rayon::slice::ParallelSliceMut;
        let row_width = self.row_width;
        self.data.par_chunks_mut(row_width)
    }
}

#[derive(Debug, Error, PartialEq)]
pub(crate) enum MatrixError {
    #[error(transparent)]
    Transpose(#[from] matrix_transpose::TransposeError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen_vec(n: u8) -> Vec<u8> {
        (0..n).collect()
    }

    #[test]
    fn test_matrix_getters() {
        let matrix = Matrix::new(gen_vec(12), 3);
        assert_eq!(matrix.row_width, 3);
        assert_eq!(matrix.rows(), 4);
        assert_eq!(matrix.len(), 12);
    }

    #[test]
    fn test_matrix_transpose() {
        let mut matrix = Matrix::new(gen_vec(128), 8);
        matrix.transpose_bits().unwrap();
        assert_eq!(matrix.row_width, 2);
        assert_eq!(matrix.rows(), 64);
        assert_eq!(matrix.len(), 128);
    }

    #[test]
    fn test_matrix_push_row() {
        let mut matrix = Matrix::new(gen_vec(12), 3);
        matrix.push_row(&[1, 2, 3]);
        assert_eq!(matrix.row_width, 3);
        assert_eq!(matrix.rows(), 5);
        assert_eq!(matrix.len(), 15);
    }

    #[test]
    fn test_matrix_split_off_rows() {
        let mut matrix = Matrix::new(gen_vec(12), 3);
        let matrix2 = matrix.split_off_rows(2);

        assert_eq!(matrix.row_width, 3);
        assert_eq!(matrix.rows(), 2);
        assert_eq!(matrix.len(), 6);

        assert_eq!(matrix2.row_width, 3);
        assert_eq!(matrix2.rows(), 2);
        assert_eq!(matrix2.len(), 6);
    }

    #[test]
    fn test_matrix_drain_rows() {
        let mut matrix = Matrix::new(gen_vec(12), 3);
        let matrix2 = matrix.drain_rows(1..3);

        assert_eq!(matrix.row_width, 3);
        assert_eq!(matrix.rows(), 2);
        assert_eq!(matrix.len(), 6);

        assert_eq!(matrix2.row_width, 3);
        assert_eq!(matrix2.rows(), 2);
        assert_eq!(matrix2.len(), 6);
    }

    #[test]
    fn test_matrix_truncate_rows() {
        let mut matrix = Matrix::new(gen_vec(12), 3);
        matrix.truncate_rows(2);

        assert_eq!(matrix.row_width, 3);
        assert_eq!(matrix.rows(), 2);
        assert_eq!(matrix.len(), 6);
    }
}
