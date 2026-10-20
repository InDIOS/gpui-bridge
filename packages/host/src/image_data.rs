//! `Image.data`: standard base64 image bytes from the protocol, decoded once and kept in a file
//! per node (PROTOCOL.md section 5).
//!
//! GPUI loads an image from a path and handles every format and animation exactly like `src`
//! (it guesses the format from the bytes, not from the extension). Writing the bytes to a file
//! keeps that behavior without decoding them ourselves.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::protocol::NodeId;

/// Number of stores created in this process, so two trees never share a directory.
static STORES: AtomicU64 = AtomicU64::new(0);

/// Decodes the `data` prop; `Err` when it is not standard base64.
pub fn decode(data: &str) -> Result<Vec<u8>, ()> {
    STANDARD.decode(data).map_err(|_| ())
}

/// The byte files of one tree, under a directory of its own in the system temp folder.
/// The directory goes away with the tree.
#[derive(Debug, Default)]
pub struct Store {
    dir: Option<PathBuf>,
}

impl Store {
    /// The file written for `node`, if there is one.
    pub fn path(&self, node: NodeId) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(node.to_string()))
    }

    /// Writes the node's bytes, replacing the previous ones.
    pub fn write(&mut self, node: NodeId, bytes: &[u8]) -> std::io::Result<PathBuf> {
        let dir = match &self.dir {
            Some(dir) => dir.clone(),
            None => {
                let dir = std::env::temp_dir().join(format!(
                    "gpui-bridge-{}-{}",
                    std::process::id(),
                    STORES.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir_all(&dir)?;
                self.dir = Some(dir.clone());
                dir
            }
        };
        let path = dir.join(node.to_string());
        fs::write(&path, bytes)?;
        Ok(path)
    }

    /// Forgets the node's file; a file that is already gone is fine.
    pub fn remove(&self, node: NodeId) {
        if let Some(path) = self.path(node)
            && let Err(err) = fs::remove_file(&path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("could not remove Image data file {}: {err}", path.display());
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(dir) = &self.dir
            && let Err(err) = fs::remove_dir_all(dir)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("could not remove {}: {err}", dir.display());
        }
    }
}
