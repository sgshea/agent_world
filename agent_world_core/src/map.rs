use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

use crate::Position;

/// Represents errors that can occur within the grid operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GridError {
    #[error("Coordinates ({x}, {y}) are out of bounds for grid size ({width}, {height})")]
    OutOfBounds {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    },
}

/// A generic 2D grid structure.
///
/// Stores elements of type `T` in a flat vector using row-major order.
/// Provides methods for accessing and modifying elements via (x, y) coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid<T> {
    width: usize,
    height: usize,
    cells: Vec<T>,
}

impl<T> Grid<T> {
    /// Creates a new grid with the specified dimensions, filled with default values.
    ///
    /// # Arguments
    ///
    /// * `width`: The width of the grid.
    /// * `height`: The height of the grid.
    ///
    /// # Panics
    ///
    /// Panics if `width * height` overflows `usize`.
    pub fn new(width: usize, height: usize) -> Self
    where
        T: Default + Clone,
    {
        let size = width.checked_mul(height).expect("Grid size overflow");
        Grid {
            width,
            height,
            cells: vec![T::default(); size],
        }
    }

    /// Creates a new grid with the specified dimensions, filled by a generator function.
    ///
    /// The generator function `f` takes `(x, y)` coordinates and returns the value for that cell.
    ///
    /// # Arguments
    ///
    /// * `width`: The width of the grid.
    /// * `height`: The height of the grid.
    /// * `f`: A function `Fn(usize, usize) -> T` to generate cell values.
    ///
    /// # Panics
    ///
    /// Panics if `width * height` overflows `usize`.
    pub fn from_generator<F>(width: usize, height: usize, mut f: F) -> Self
    where
        F: FnMut(usize, usize) -> T,
    {
        let size = width.checked_mul(height).expect("Grid size overflow");
        let mut cells = Vec::with_capacity(size);
        for y in 0..height {
            for x in 0..width {
                cells.push(f(x, y));
            }
        }
        Grid {
            width,
            height,
            cells,
        }
    }

    /// Returns the width of the grid.
    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the grid.
    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Converts (x, y) coordinates to a flat vector index.
    ///
    /// Returns `None` if the coordinates are out of bounds.
    #[inline]
    pub fn coords_to_index(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(y * self.width + x)
        } else {
            None
        }
    }

    /// Converts a flat vector index back to (x, y) coordinates.
    ///
    /// Returns `None` if the index is out of bounds.
    #[inline]
    pub fn index_to_coords(&self, index: usize) -> Option<(usize, usize)> {
        if index < self.cells.len() {
            let y = index / self.width;
            let x = index % self.width;
            Some((x, y))
        } else {
            None
        }
    }

    /// Checks if the given coordinates are within the grid boundaries.
    #[inline]
    pub fn is_valid(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    /// Gets an immutable reference to the cell at the given coordinates.
    ///
    /// Returns `None` if the coordinates are out of bounds.
    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        if self.is_valid(x, y) {
            let index = self.coords_to_index(x, y)?;
            self.cells.get(index) // Should always be Some if coords_to_index returned Some
        } else {
            None
        }
    }

    /// Gets a mutable reference to the cell at the given coordinates.
    ///
    /// Returns `None` if the coordinates are out of bounds.
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut T> {
        if self.is_valid(x, y) {
            let index = self.coords_to_index(x, y)?;
            self.cells.get_mut(index) // Should always be Some if coords_to_index returned Some
        } else {
            None
        }
    }

    /// Sets the value of the cell at the given coordinates.
    ///
    /// Returns `Ok(())` on success, or `Err(GridError::OutOfBounds)` if the
    /// coordinates are invalid.
    pub fn set(&mut self, x: usize, y: usize, value: T) -> Result<(), GridError> {
        let index = self.coords_to_index(x, y).ok_or(GridError::OutOfBounds {
            x,
            y,
            width: self.width,
            height: self.height,
        })?;
        self.cells[index] = value;
        Ok(())
    }

    /// Returns an iterator over the cells of the grid in row-major order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.cells.iter()
    }

    /// Returns a mutable iterator over the cells of the grid in row-major order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.cells.iter_mut()
    }

    /// Returns an iterator that yields `((x, y), &T)` for each cell.
    pub fn enumerate(&self) -> impl Iterator<Item = ((usize, usize), &T)> {
        self.cells
            .iter()
            .enumerate()
            .map(move |(index, cell)| (self.index_to_coords(index).unwrap(), cell))
    }

    /// Returns a mutable iterator that yields `((x, y), &mut T)` for each cell.
    pub fn enumerate_mut(&mut self) -> impl Iterator<Item = ((usize, usize), &mut T)> {
        let width = self.width; // Capture width for the closure
        self.cells.iter_mut().enumerate().map(move |(index, cell)| {
            let y = index / width;
            let x = index % width;
            ((x, y), cell)
        })
    }

    /// Returns a slice containing all cells in the grid.
    pub fn as_slice(&self) -> &[T] {
        &self.cells
    }

    /// Returns a mutable slice containing all cells in the grid.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.cells
    }
}

/// Allows indexing the grid using `(usize, usize)` coordinates for immutable access.
impl<T> Index<(usize, usize)> for Grid<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (x, y) = index;
        match self.coords_to_index(x, y) {
            Some(idx) => &self.cells[idx],
            None => panic!(
                "Grid index ({}, {}) out of bounds for grid size ({}, {})",
                x, y, self.width, self.height
            ),
        }
    }
}

/// Allows indexing the grid using `(usize, usize)` coordinates for mutable access.
impl<T> IndexMut<(usize, usize)> for Grid<T> {
    #[inline]
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (x, y) = index;
        let width = self.width;
        let height = self.height;
        match self.coords_to_index(x, y) {
            Some(idx) => &mut self.cells[idx],
            None => panic!(
                "Grid index ({}, {}) out of bounds for grid size ({}, {})",
                x, y, width, height
            ),
        }
    }
}

/// Indexing using Position coordinates for access
impl<T> Index<Position> for Grid<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: Position) -> &Self::Output {
        let (x, y) = (index.x, index.y);
        match self.coords_to_index(x, y) {
            Some(idx) => &self.cells[idx],
            None => panic!(
                "Grid index ({}, {}) out of bounds for grid size ({}, {})",
                x, y, self.width, self.height
            ),
        }
    }
}

/// Indexing using Position coordinates for mutable access
impl<T> IndexMut<Position> for Grid<T> {
    #[inline]
    fn index_mut(&mut self, index: Position) -> &mut Self::Output {
        let (x, y) = (index.x, index.y);
        let width = self.width;
        let height = self.height;
        match self.coords_to_index(x, y) {
            Some(idx) => &mut self.cells[idx],
            None => panic!(
                "Grid index ({}, {}) out of bounds for grid size ({}, {})",
                x, y, width, height
            ),
        }
    }
}
