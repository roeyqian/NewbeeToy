use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

const DAT_MAGIC: &[u8; 8] = b"NBTDAT01";

pub fn encode_binary_dat<T: Serialize>(data: &T) -> Result<Vec<u8>, String> {
    let payload = bincode::serde::encode_to_vec(data, bincode::config::standard())
        .map_err(|err| err.to_string())?;
    let mut bytes = Vec::with_capacity(DAT_MAGIC.len() + payload.len());
    bytes.extend_from_slice(DAT_MAGIC);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub fn decode_binary_dat<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    if !bytes.starts_with(DAT_MAGIC) {
        return Err("missing NewbeeToy dat header".to_string());
    }

    let payload = &bytes[DAT_MAGIC.len()..];
    let (data, bytes_read) =
        bincode::serde::decode_from_slice(payload, bincode::config::standard())
            .map_err(|err| err.to_string())?;
    if bytes_read != payload.len() {
        return Err("unexpected trailing bytes in NewbeeToy dat file".to_string());
    }

    Ok(data)
}

pub fn write_binary_dat_path<T: Serialize>(path: &Path, data: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let bytes = encode_binary_dat(data)?;
    std::fs::write(path, bytes).map_err(|err| err.to_string())
}
