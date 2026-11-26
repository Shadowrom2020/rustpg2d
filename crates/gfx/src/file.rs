//! Reading files: from the disk, or on the web from the server beside the page.

use crate::Error;

/// Reads a whole file (a future: the web fetches it).
pub async fn load_file(path: &str) -> Result<Vec<u8>, Error> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(path).map_err(|e| Error::File(format!("{path}: {e}")))
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::fetch(path).await
    }
}
