//! Repository-owned bounded child for the local dataset-prepare vertical slice.

#![forbid(unsafe_code)]

use std::io::{Read as _, Write as _};

use colibri_local_worker::{
    DatasetExecutorInputV1, MAX_LOCAL_EXECUTOR_REQUEST_BYTES, execute_dataset_prepare_child,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--dataset-prepare-v1"] {
        return Err("expected exactly --dataset-prepare-v1".into());
    }
    if std::env::var_os("MOJOCOLIB_TEST_SECRET").is_some() {
        return Err("executor unexpectedly received a parent secret".into());
    }

    let limit = u64::try_from(MAX_LOCAL_EXECUTOR_REQUEST_BYTES + 1)?;
    let mut input = Vec::with_capacity(MAX_LOCAL_EXECUTOR_REQUEST_BYTES + 1);
    std::io::stdin().take(limit).read_to_end(&mut input)?;
    if input.len() > MAX_LOCAL_EXECUTOR_REQUEST_BYTES {
        return Err("executor request exceeds the local bound".into());
    }
    let request: DatasetExecutorInputV1 = serde_json::from_slice(&input)?;
    let output = execute_dataset_prepare_child(request)?;
    std::io::stdout().write_all(&output)?;
    Ok(())
}
