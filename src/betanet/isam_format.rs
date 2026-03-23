//! Exact port of Kotlin ISAM format from DayJobTest.kt and ISAMCursor.kt
//!
//! Provides binary-compatible file format with network-endian encoding
//! and exact meta file format matching the Kotlin implementation.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write, BufRead, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};

/// Exact port of IOMemento enum from Kotlin with precise network sizes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IOMemento {
    IoBoolean = 0,  // 1 byte
    IoByte = 1,     // 1 byte  
    IoInt = 2,      // 4 bytes
    IoLong = 3,     // 8 bytes
    IoFloat = 4,    // 4 bytes
    IoDouble = 5,   // 8 bytes
    IoString = 6,   // variable
    IoLocalDate = 7, // 8 bytes (days since epoch)
    IoInstant = 8,  // 12 bytes (8 bytes seconds + 4 bytes nanos)
    IoNothing = 9,  // 0 bytes
}

impl IOMemento {
    /// Get network size in bytes (exact match to Kotlin IOMemento.networkSize)
    pub fn network_size(&self) -> Option<usize> {
        match self {
            IOMemento::IoBoolean => Some(1),
            IOMemento::IoByte => Some(1),
            IOMemento::IoInt => Some(4),
            IOMemento::IoLong => Some(8),
            IOMemento::IoFloat => Some(4),
            IOMemento::IoDouble => Some(8),
            IOMemento::IoString => None, // Variable size
            IOMemento::IoLocalDate => Some(8),
            IOMemento::IoInstant => Some(12),
            IOMemento::IoNothing => Some(0),
        }
    }

    /// Parse IOMemento from string name (matching Kotlin valueOf)
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "IoBoolean" => Some(IOMemento::IoBoolean),
            "IoByte" => Some(IOMemento::IoByte),
            "IoInt" => Some(IOMemento::IoInt),
            "IoLong" => Some(IOMemento::IoLong),
            "IoFloat" => Some(IOMemento::IoFloat),
            "IoDouble" => Some(IOMemento::IoDouble),
            "IoString" => Some(IOMemento::IoString),
            "IoLocalDate" => Some(IOMemento::IoLocalDate),
            "IoInstant" => Some(IOMemento::IoInstant),
            "IoNothing" => Some(IOMemento::IoNothing),
            _ => None,
        }
    }

    /// Get string name (matching Kotlin name property)
    pub fn name(&self) -> &'static str {
        match self {
            IOMemento::IoBoolean => "IoBoolean",
            IOMemento::IoByte => "IoByte",
            IOMemento::IoInt => "IoInt",
            IOMemento::IoLong => "IoLong",
            IOMemento::IoFloat => "IoFloat",
            IOMemento::IoDouble => "IoDouble",
            IOMemento::IoString => "IoString",
            IOMemento::IoLocalDate => "IoLocalDate",
            IOMemento::IoInstant => "IoInstant",
            IOMemento::IoNothing => "IoNothing",
        }
    }
}

/// Scalar type with IOMemento and optional string description
/// Exact port of Kotlin Scalar companion object
#[derive(Debug, Clone)]
pub struct Scalar {
    pub io_memento: IOMemento,
    pub description: Option<String>,
}

impl Scalar {
    pub fn new(io_memento: IOMemento, description: Option<String>) -> Self {
        Self { io_memento, description }
    }
}

/// Network coordinate pair (start, end) for column layout
/// Exact match to Kotlin Vect02<Int, Int> for wcoords
pub type NetworkCoord = (usize, usize);

/// Calculate network coordinates from IOMemento types
/// Exact port of networkCoords() function from Cursor.kt
pub fn network_coords(
    io_mementos: &[IOMemento],
    default_varchar_size: usize,
    varchar_sizes: Option<&HashMap<usize, usize>>,
) -> Vec<NetworkCoord> {
    let sizes = network_sizes(io_mementos, default_varchar_size, varchar_sizes);
    
    let mut record_len = 0;
    let mut coords = Vec::with_capacity(sizes.len());
    
    for &size in &sizes {
        let start = record_len;
        let end = record_len + size;
        coords.push((start, end));
        record_len = end;
    }
    
    coords
}

/// Calculate network sizes for each column
/// Matching networkSizes() logic from Kotlin
pub fn network_sizes(
    io_mementos: &[IOMemento],
    default_varchar_size: usize,
    varchar_sizes: Option<&HashMap<usize, usize>>,
) -> Vec<usize> {
    io_mementos.iter().enumerate().map(|(index, &memento)| {
        match memento.network_size() {
            Some(size) => size,
            None => {
                // Variable size (IoString) - check varchar_sizes map
                varchar_sizes
                    .and_then(|map| map.get(&index))
                    .copied()
                    .unwrap_or(default_varchar_size)
            }
        }
    }).collect()
}

/// Write ISAM meta file with exact format from Kotlin writeISAMMeta()
/// Format:
/// Line 1: Space-separated coordinate pairs (start end start end ...)
/// Line 2: Space-separated column names (underscores for spaces)  
/// Line 3: Space-separated IOMemento names
pub fn write_isam_meta(
    pathname: &str,
    coords: &[NetworkCoord],
    column_names: &[String],
    io_mementos: &[IOMemento],
) -> Result<(), std::io::Error> {
    let meta_path = format!("{}.meta", pathname);
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(meta_path)?;
    
    let mut writer = BufWriter::new(file);
    
    // Write header comments
    writeln!(writer, "# format:  coords WS .. EOL names WS .. EOL TypeMememento WS ..")?;
    writeln!(writer, "# last coord is the recordlen")?;
    
    // Line 1: Coordinates flattened as "start end start end ..."
    let coord_line: Vec<String> = coords.iter()
        .flat_map(|&(start, end)| vec![start.to_string(), end.to_string()])
        .collect();
    writeln!(writer, "{}", coord_line.join(" "))?;
    
    // Line 2: Column names with spaces replaced by underscores
    let name_line: Vec<String> = column_names.iter()
        .map(|name| name.replace(' ', "_"))
        .collect();
    writeln!(writer, "{}", name_line.join(" "))?;
    
    // Line 3: IOMemento names
    let memento_line: Vec<String> = io_mementos.iter()
        .map(|memento| memento.name().to_string())
        .collect();
    writeln!(writer, "{}", memento_line.join(" "))?;
    
    writer.flush()?;
    Ok(())
}

/// Read ISAM meta file and parse coordinates, names, and types
/// Exact port of ISAMCursor init logic from Kotlin
pub fn read_isam_meta(meta_path: &str) -> Result<(Vec<NetworkCoord>, Vec<String>, Vec<IOMemento>), std::io::Error> {
    let file = File::open(meta_path)?;
    let reader = BufReader::new(file);
    
    let mut lines: Vec<String> = reader.lines()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|line| !line.starts_with("# ") && !line.trim().is_empty())
        .collect();
    
    if lines.len() < 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Meta file must have at least 3 non-comment lines"
        ));
    }
    
    // Parse coordinates from line 1
    let coord_nums: Vec<usize> = lines[0]
        .split_whitespace()
        .map(|s| s.parse())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    if coord_nums.len() % 2 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Coordinate count must be even (start/end pairs)"
        ));
    }
    
    let coords: Vec<NetworkCoord> = coord_nums
        .chunks(2)
        .map(|chunk| (chunk[0], chunk[1]))
        .collect();
    
    // Parse column names from line 2
    let names: Vec<String> = lines[1]
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    
    // Parse IOMemento types from line 3
    let mementos: Vec<IOMemento> = lines[2]
        .split_whitespace()
        .map(|s| IOMemento::from_name(s).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown IOMemento: {}", s))
        }))
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok((coords, names, mementos))
}

/// Cell data value matching Kotlin Any? semantics
#[derive(Debug, Clone)]
pub enum CellValue {
    Boolean(bool),
    Byte(i8),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    LocalDate(i64), // Days since Unix epoch
    Instant { seconds: i64, nanos: u32 }, // Kotlin Instant representation
    Nothing,
}

impl CellValue {
    /// Write value to buffer using network-endian encoding
    /// Matching Kotlin ByteBuffer.put() semantics with network byte order
    pub fn write_to_buffer(&self, buffer: &mut Vec<u8>, memento: IOMemento, varchar_size: Option<usize>) -> Result<(), std::io::Error> {
        match (self, memento) {
            (CellValue::Boolean(b), IOMemento::IoBoolean) => {
                buffer.write_u8(if *b { 1 } else { 0 })?;
            }
            (CellValue::Byte(b), IOMemento::IoByte) => {
                buffer.write_i8(*b)?;
            }
            (CellValue::Int(i), IOMemento::IoInt) => {
                buffer.write_i32::<BigEndian>(*i)?;
            }
            (CellValue::Long(l), IOMemento::IoLong) => {
                buffer.write_i64::<BigEndian>(*l)?;
            }
            (CellValue::Float(f), IOMemento::IoFloat) => {
                buffer.write_f32::<BigEndian>(*f)?;
            }
            (CellValue::Double(d), IOMemento::IoDouble) => {
                buffer.write_f64::<BigEndian>(*d)?;
            }
            (CellValue::String(s), IOMemento::IoString) => {
                let size = varchar_size.unwrap_or(128);
                let bytes = s.as_bytes();
                let write_len = std::cmp::min(bytes.len(), size);
                
                // Write string bytes
                buffer.extend_from_slice(&bytes[..write_len]);
                
                // Pad with spaces (ASCII 32) to fill varchar_size
                for _ in write_len..size {
                    buffer.write_u8(32)?; // SPACE byte from Kotlin
                }
            }
            (CellValue::LocalDate(days), IOMemento::IoLocalDate) => {
                buffer.write_i64::<BigEndian>(*days)?;
            }
            (CellValue::Instant { seconds, nanos }, IOMemento::IoInstant) => {
                buffer.write_i64::<BigEndian>(*seconds)?;
                buffer.write_u32::<BigEndian>(*nanos)?;
            }
            (CellValue::Nothing, IOMemento::IoNothing) => {
                // Write nothing
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Mismatched value type and IOMemento: {:?} vs {:?}", self, memento)
                ));
            }
        }
        Ok(())
    }
    
    /// Create Instant from current time (matching Kotlin Instant.now())
    pub fn now_instant() -> Self {
        let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        CellValue::Instant {
            seconds: duration.as_secs() as i64,
            nanos: duration.subsec_nanos(),
        }
    }
}

/// Row of cell values with scalars (matching Kotlin RowVec semantics)
#[derive(Debug, Clone)]
pub struct RowVec {
    pub cells: Vec<CellValue>,
    pub scalars: Vec<Scalar>,
}

impl RowVec {
    pub fn new(cells: Vec<CellValue>, scalars: Vec<Scalar>) -> Self {
        Self { cells, scalars }
    }
    
    pub fn size(&self) -> usize {
        self.cells.len()
    }
}

/// Cursor trait matching Kotlin Cursor interface
/// Using Kotlin's Pai2<Int, (Int) -> RowVec> semantics
pub trait Cursor {
    fn size(&self) -> usize;
    fn get_row(&self, index: usize) -> Option<RowVec>;
    fn scalars(&self) -> &[Scalar];
}

/// Write cursor data to ISAM format
/// Exact port of Cursor.writeISAM() from Kotlin
pub fn write_isam<C: Cursor>(
    cursor: &C,
    pathname: &str,
    default_varchar_size: usize,
    varchar_sizes: Option<&HashMap<usize, usize>>,
) -> Result<(), std::io::Error> {
    let scalars = cursor.scalars();
    let io_mementos: Vec<IOMemento> = scalars.iter()
        .map(|scalar| scalar.io_memento)
        .collect();
    
    let column_names: Vec<String> = scalars.iter()
        .map(|scalar| scalar.description.clone().unwrap_or_else(|| "unknown".to_string()))
        .collect();
    
    // Calculate network coordinates
    let coords = network_coords(&io_mementos, default_varchar_size, varchar_sizes);
    let record_len = coords.last().map(|(_, end)| *end).unwrap_or(0);
    
    // Write meta file
    write_isam_meta(pathname, &coords, &column_names, &io_mementos)?;
    
    // Write binary data
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(pathname)?;
    
    let mut writer = BufWriter::new(file);
    
    for row_index in 0..cursor.size() {
        if let Some(row) = cursor.get_row(row_index) {
            let mut row_buffer = Vec::with_capacity(record_len);
            
            for (col_index, cell) in row.cells.iter().enumerate() {
                let memento = io_mementos[col_index];
                let varchar_size = if memento == IOMemento::IoString {
                    varchar_sizes.and_then(|map| map.get(&col_index)).copied()
                } else {
                    None
                };
                
                cell.write_to_buffer(&mut row_buffer, memento, varchar_size)?;
            }
            
            // Ensure row is exactly record_len bytes
            row_buffer.resize(record_len, 0);
            writer.write_all(&row_buffer)?;
        }
    }
    
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_io_memento_sizes() {
        assert_eq!(IOMemento::IoBoolean.network_size(), Some(1));
        assert_eq!(IOMemento::IoInt.network_size(), Some(4));
        assert_eq!(IOMemento::IoLong.network_size(), Some(8));
        assert_eq!(IOMemento::IoFloat.network_size(), Some(4));
        assert_eq!(IOMemento::IoDouble.network_size(), Some(8));
        assert_eq!(IOMemento::IoInstant.network_size(), Some(12));
        assert_eq!(IOMemento::IoString.network_size(), None);
    }
    
    #[test]
    fn test_network_coords() {
        let mementos = vec![IOMemento::IoInt, IOMemento::IoFloat, IOMemento::IoLong];
        let coords = network_coords(&mementos, 128, None);
        
        assert_eq!(coords, vec![(0, 4), (4, 8), (8, 16)]);
    }
    
    #[test]
    fn test_cell_value_write() {
        let mut buffer = Vec::new();
        let value = CellValue::Int(42);
        value.write_to_buffer(&mut buffer, IOMemento::IoInt, None).unwrap();
        
        // Should be 4 bytes in big-endian format
        assert_eq!(buffer, vec![0, 0, 0, 42]);
    }
}