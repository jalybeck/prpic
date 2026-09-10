use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

// Pinned pdfium-binaries release: https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F8044
const PDFIUM_VERSION_TAG: &str = "chromium/8044";

struct PdfiumAsset {
    archive_name: &'static str,
    archive_sha256: &'static str,
    lib_path_in_archive: &'static str,
    lib_file_name: &'static str,
}

const WINDOWS_ASSET: PdfiumAsset = PdfiumAsset {
    archive_name: "pdfium-win-x64.tgz",
    archive_sha256: "78a17d9a5f14467631c26a3ac8741b27a0471ecc05bd6a119b523598160a0537",
    lib_path_in_archive: "bin/pdfium.dll",
    lib_file_name: "pdfium.dll",
};

const LINUX_ASSET: PdfiumAsset = PdfiumAsset {
    archive_name: "pdfium-linux-x64.tgz",
    archive_sha256: "eb142f416aed3a72fc5a02dbd5884868a16cb99dc0cf53e6bdd64afbf67b05f4",
    lib_path_in_archive: "lib/libpdfium.so",
    lib_file_name: "libpdfium.so",
};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let asset = match target_os.as_str() {
        "windows" => &WINDOWS_ASSET,
        "linux" => &LINUX_ASSET,
        // Other platforms fall back to a system-installed pdfium library at runtime.
        _ => return,
    };

    // OUT_DIR is target/<profile>/build/<pkg>-<hash>/out; the binary itself lands in target/<profile>.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR should be nested three levels under the profile directory")
        .to_path_buf();
    let target_dir = profile_dir
        .parent()
        .expect("profile directory has a parent target directory");

    let lib_dest = profile_dir.join(asset.lib_file_name);
    let license_dest_dir = profile_dir.join("THIRD_PARTY_LICENSES").join("pdfium");
    if lib_dest.exists() && license_dest_dir.join("LICENSE").exists() {
        return;
    }

    let cache_dir = target_dir
        .join("pdfium-cache")
        .join(PDFIUM_VERSION_TAG.replace('/', "-"));
    fs::create_dir_all(&cache_dir).expect("failed to create pdfium cache directory");

    let archive_bytes = fetch_archive(asset, &cache_dir);
    extract_assets(&archive_bytes, asset, &lib_dest, &license_dest_dir);
}

fn fetch_archive(asset: &PdfiumAsset, cache_dir: &Path) -> Vec<u8> {
    let cached_path = cache_dir.join(asset.archive_name);
    if let Ok(bytes) = fs::read(&cached_path) {
        if sha256_hex(&bytes) == asset.archive_sha256 {
            return bytes;
        }
    }

    let url = format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/download/{}/{}",
        PDFIUM_VERSION_TAG.replace('/', "%2F"),
        asset.archive_name
    );

    let response = ureq::get(&url)
        .call()
        .unwrap_or_else(|e| panic!("failed to download {url}: {e}"));
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("failed to read response body from {url}: {e}"));

    let actual = sha256_hex(&bytes);
    assert_eq!(
        actual, asset.archive_sha256,
        "downloaded {} checksum mismatch: expected {}, got {actual}",
        asset.archive_name, asset.archive_sha256
    );

    fs::write(&cached_path, &bytes).expect("failed to cache downloaded pdfium archive");
    bytes
}

fn extract_assets(
    archive_bytes: &[u8],
    asset: &PdfiumAsset,
    lib_dest: &Path,
    license_dest_dir: &Path,
) {
    fs::create_dir_all(license_dest_dir.join("licenses"))
        .expect("failed to create pdfium license directory");

    let decoder = flate2::read::GzDecoder::new(archive_bytes);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().expect("failed to read pdfium archive") {
        let mut entry = entry.expect("failed to read pdfium archive entry");
        let path = entry
            .path()
            .expect("invalid path in pdfium archive")
            .to_string_lossy()
            .into_owned();

        if path == asset.lib_path_in_archive {
            entry
                .unpack(lib_dest)
                .expect("failed to extract pdfium library");
        } else if path == "LICENSE" {
            entry
                .unpack(license_dest_dir.join("LICENSE"))
                .expect("failed to extract pdfium LICENSE");
        } else if let Some(name) = path.strip_prefix("licenses/") {
            if !name.is_empty() {
                entry
                    .unpack(license_dest_dir.join("licenses").join(name))
                    .expect("failed to extract pdfium license file");
            }
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

