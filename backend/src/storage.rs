use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use std::path::PathBuf;
use std::sync::Arc;

/// Build an ObjectStore from environment variables, or `None` if storage is disabled.
///
/// - Unset / empty `STORAGE_BACKEND`: storage disabled (default)
/// - `STORAGE_BACKEND=local`: stores files under `STORAGE_PATH` (default: `./uploads`)
/// - `STORAGE_BACKEND=s3`: uses S3 with `S3_BUCKET`, `S3_REGION`, and standard AWS credential chain
pub fn build_object_store() -> Result<Option<Arc<dyn ObjectStore>>, Box<dyn std::error::Error>> {
    let backend = std::env::var("STORAGE_BACKEND").unwrap_or_default();

    if backend.is_empty() {
        tracing::info!("Storage backend: disabled (set STORAGE_BACKEND to enable file uploads)");
        return Ok(None);
    }

    match backend.as_str() {
        "s3" => {
            let bucket =
                std::env::var("S3_BUCKET").expect("S3_BUCKET must be set when STORAGE_BACKEND=s3");
            let region =
                std::env::var("S3_REGION").expect("S3_REGION must be set when STORAGE_BACKEND=s3");

            let mut builder = AmazonS3Builder::from_env()
                .with_bucket_name(bucket)
                .with_region(region);

            if let Ok(endpoint) = std::env::var("S3_ENDPOINT") {
                builder = builder.with_endpoint(endpoint).with_allow_http(true);
            }

            let store = builder.build()?;
            tracing::info!("Storage backend: S3");
            Ok(Some(Arc::new(store)))
        }
        "local" => {
            let path = std::env::var("STORAGE_PATH").unwrap_or_else(|_| "./uploads".to_string());
            let path = PathBuf::from(&path);
            std::fs::create_dir_all(&path)?;
            let store = LocalFileSystem::new_with_prefix(&path)?;
            tracing::info!("Storage backend: local filesystem at {}", path.display());
            Ok(Some(Arc::new(store)))
        }
        other => {
            Err(format!("Unknown STORAGE_BACKEND: '{other}' (expected 'local' or 's3')").into())
        }
    }
}
