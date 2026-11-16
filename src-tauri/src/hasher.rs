use anyhow::{Context, Result};
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
    pub murmur2: u32,
    pub file_size: u64,
}

/// Calcula SHA-1, SHA-512 (Modrinth API) e Murmur2 Fingerprint (CurseForge API)
/// em uma única leitura em memória do arquivo.
pub fn compute_file_hashes(path: &Path) -> Result<FileHashes> {
    let bytes = fs::read(path)
        .with_context(|| format!("Falha ao ler arquivo para hash: {}", path.display()))?;

    let file_size = bytes.len() as u64;

    let mut sha1_hasher = Sha1::new();
    sha1_hasher.update(&bytes);
    let sha1 = hex::encode(sha1_hasher.finalize());

    let mut sha512_hasher = Sha512::new();
    sha512_hasher.update(&bytes);
    let sha512 = hex::encode(sha512_hasher.finalize());

    let murmur2 = compute_curseforge_murmur2(&bytes);

    Ok(FileHashes {
        sha1,
        sha512,
        murmur2,
        file_size,
    })
}

/// Implementação oficial do algoritmo MurmurHash2 de 32 bits usado pelo CurseForge.
/// Remove caracteres de espaço em branco (0x09 \t, 0x0A \n, 0x0D \r, 0x20 espaço)
/// antes de aplicar o Murmur2 com seed = 1 e constante m = 0x5bd1e995.
pub fn compute_curseforge_murmur2(data: &[u8]) -> u32 {
    let normalized: Vec<u8> = data
        .iter()
        .copied()
        .filter(|&b| b != 9 && b != 10 && b != 13 && b != 32)
        .collect();

    const M: u32 = 0x5bd1e995;
    const R: u32 = 24;
    let length = normalized.len() as u32;
    let mut h: u32 = 1 ^ length;

    let chunks = normalized.chunks_exact(4);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);

        h = h.wrapping_mul(M);
        h ^= k;
    }

    match remainder.len() {
        3 => {
            h ^= (remainder[2] as u32) << 16;
            h ^= (remainder[1] as u32) << 8;
            h ^= remainder[0] as u32;
            h = h.wrapping_mul(M);
        }
        2 => {
            h ^= (remainder[1] as u32) << 8;
            h ^= remainder[0] as u32;
            h = h.wrapping_mul(M);
        }
        1 => {
            h ^= remainder[0] as u32;
            h = h.wrapping_mul(M);
        }
        _ => {}
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;

    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curseforge_murmur2_whitespace_stripping() {
        let a = b"hello world\n\r\t!";
        let b = b"helloworld!";
        assert_eq!(compute_curseforge_murmur2(a), compute_curseforge_murmur2(b));
    }
}
