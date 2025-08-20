pub mod format;

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tracing::{debug, info};

pub use format::{Snapshot, SnapshotMetadata};

/// Error type for snapshot operations
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Compression error: {0}")]
    CompressionError(String),
    
    #[error("Invalid snapshot format")]
    InvalidFormat,
    
    #[error("Snapshot version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
}

pub type SnapshotResult<T> = Result<T, SnapshotError>;

/// Snapshot manager for saving and loading snapshots
pub struct SnapshotManager {
    compress: bool,
}

impl SnapshotManager {
    pub fn new() -> Self {
        Self { compress: true }
    }
    
    pub fn with_compression(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }
    
    /// Save a snapshot to a file
    pub fn save(&self, snapshot: &Snapshot, path: &Path) -> SnapshotResult<()> {
        info!("Saving snapshot to {:?}", path);
        
        // Serialize to JSON
        let json_data = serde_json::to_vec_pretty(snapshot)?;
        
        if self.compress {
            // Compress using gzip
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&json_data)?;
            let compressed = encoder.finish()?;
            
            // Write compressed data
            let mut file = File::create(path)?;
            file.write_all(&compressed)?;
            
            debug!("Snapshot compressed from {} to {} bytes", 
                   json_data.len(), compressed.len());
        } else {
            // Write uncompressed JSON
            let mut file = File::create(path)?;
            file.write_all(&json_data)?;
        }
        
        info!("Snapshot saved successfully");
        Ok(())
    }
    
    /// Load a snapshot from a file
    pub fn load(&self, path: &Path) -> SnapshotResult<Snapshot> {
        info!("Loading snapshot from {:?}", path);
        
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        // Try to decompress first
        let json_data = if self.is_compressed(&buffer) {
            let mut decoder = flate2::read::GzDecoder::new(&buffer[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        } else {
            buffer
        };
        
        // Deserialize from JSON
        let snapshot: Snapshot = serde_json::from_slice(&json_data)?;
        
        // Validate version
        if snapshot.version != format::SNAPSHOT_VERSION {
            return Err(SnapshotError::VersionMismatch {
                expected: format::SNAPSHOT_VERSION.to_string(),
                actual: snapshot.version.clone(),
            });
        }
        
        info!("Snapshot loaded successfully");
        Ok(snapshot)
    }
    
    /// Check if data is gzip compressed
    fn is_compressed(&self, data: &[u8]) -> bool {
        // Gzip magic numbers: 1f 8b
        data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
    }
    
    /// Create a snapshot archive with multiple files
    pub fn create_archive(&self, snapshot: &Snapshot, reports: HashMap<String, Vec<u8>>, path: &Path) -> SnapshotResult<()> {
        use tar::Builder;
        
        info!("Creating snapshot archive at {:?}", path);
        
        let file = File::create(path)?;
        let mut archive = Builder::new(file);
        
        // Add snapshot JSON
        let snapshot_json = serde_json::to_vec_pretty(snapshot)?;
        let mut header = tar::Header::new_gnu();
        header.set_path("snapshot.json")?;
        header.set_size(snapshot_json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append(&header, &snapshot_json[..])?;
        
        // Add reports
        for (name, data) in reports {
            let mut header = tar::Header::new_gnu();
            header.set_path(&name)?;
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append(&header, &data[..])?;
        }
        
        archive.finish()?;
        info!("Snapshot archive created successfully");
        
        Ok(())
    }
    
    /// Extract a snapshot archive
    pub fn extract_archive(&self, path: &Path, output_dir: &Path) -> SnapshotResult<Snapshot> {
        use tar::Archive;
        
        info!("Extracting snapshot archive from {:?}", path);
        
        let file = File::open(path)?;
        let mut archive = Archive::new(file);
        
        // Extract all files
        archive.unpack(output_dir)?;
        
        // Load the snapshot
        let snapshot_path = output_dir.join("snapshot.json");
        self.load(&snapshot_path)
    }
}
