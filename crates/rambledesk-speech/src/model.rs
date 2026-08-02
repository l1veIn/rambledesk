use reqwest::{
    StatusCode,
    blocking::Client,
    header::{ACCEPT_ENCODING, RANGE},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub const MODEL_ID: &str = "x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05";
pub const MODEL_DIR: &str = "sherpa-x-asr";
const MODEL_BYTES: u64 = 169_347_218;
const ARCHIVE_BYTES: u64 = 133_895_136;
const ARCHIVE_SHA256: &str = "fa5f63d618e5a01526e275a358bb7772e403f84808a4769fba52cffd8160bf74";
const ARCHIVE_URLS: &[&str] = &[
    "https://ghfast.top/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2",
];
const MIRRORS: &[(&str, &str)] = &[
    (
        "https://www.modelscope.cn/models/yangchen1258/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05",
        "aa50faf0a9e45f6ea8913762151d47679ba468d7",
    ),
    (
        "https://www.modelscope.cn/models/bujidc/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05",
        "57f0a27e56d43b36f350be2ecc4a2200232d13e7",
    ),
];
const FILES: &[(&str, u64, &str)] = &[
    (
        "encoder.int8.onnx",
        155_278_641,
        "908596dcc137a73b95be908ca55e88caa1b3dbbe8027c171615f4b0609c5eb1e",
    ),
    (
        "decoder.onnx",
        11_309_084,
        "a1cbc9eac2d5e3fb6617a218c67ad6daaa7f4e0fd225f08b2c22ab0413c8c257",
    ),
    (
        "joiner.int8.onnx",
        2_581_422,
        "aedb7fa697b2ab43f20499826fff7c997eea7d67db77be97769aeeeb726e63b3",
    ),
    (
        "tokens.txt",
        58_806,
        "b818a60878b9aae978cbb8ad594acbd403d76d1af2e31ef4197c84e2dbdba27c",
    ),
    (
        "bpe.model",
        119_265,
        "f87a38025a5fdd1e4e9591f6a44bb81295097ce0b80df6f4ab9f44e52c64ca5f",
    ),
];

#[derive(Debug, Clone, Serialize)]
pub struct SpeechModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub size_bytes: u64,
    pub installed: bool,
    pub path: String,
    pub missing_files: Vec<String>,
}

pub fn model_dir(library_root: &Path) -> PathBuf {
    library_root.join("models").join("speech").join(MODEL_DIR)
}

pub fn model_info(library_root: &Path) -> SpeechModelInfo {
    let dir = model_dir(library_root);
    let missing_files = FILES
        .iter()
        .filter_map(|(name, size, _)| match fs::metadata(dir.join(name)) {
            Ok(md) if md.len() == *size => None,
            Ok(_) => Some(format!("{name}（大小不符）")),
            Err(_) => Some(format!("{name}（缺失）")),
        })
        .collect::<Vec<_>>();
    SpeechModelInfo {
        id: MODEL_ID,
        display_name: "X-ASR 流式中英标点（int8，480ms）",
        size_bytes: MODEL_BYTES,
        installed: missing_files.is_empty(),
        path: {
            let value = dir.to_string_lossy();
            value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
        },
        missing_files,
    }
}

pub fn delete_model(library_root: &Path) -> Result<(), String> {
    let dir = model_dir(library_root);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("删除模型 {} 失败：{e}", dir.display()))?;
    }
    Ok(())
}

pub fn download_model(library_root: &Path, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let dir = model_dir(library_root);
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建模型目录：{e}"))?;
    if model_info(library_root).installed {
        progress(MODEL_BYTES, MODEL_BYTES);
        return Ok(());
    }
    let mirror_error = download_from_mirrors(&dir, progress).err();
    if model_info(library_root).installed {
        return Ok(());
    }
    download_archive(library_root, &dir, progress).map_err(|official| match mirror_error {
        Some(mirror) => format!("ModelScope 镜像失败：{mirror}；GitHub 源也失败：{official}"),
        None => official,
    })?;
    if !model_info(library_root).installed {
        return Err("模型下载完成，但完整性检查未通过".into());
    }
    Ok(())
}

fn download_from_mirrors(dir: &Path, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let mut done = 0;
    for (name, size, sha) in FILES {
        let dest = dir.join(name);
        if fs::metadata(&dest).is_ok_and(|m| m.len() == *size) {
            done += size;
            progress(done, MODEL_BYTES);
            continue;
        }
        let base = done;
        let mut errors = Vec::new();
        for (mirror, revision) in MIRRORS {
            let url = format!("{mirror}/resolve/{revision}/{name}");
            match download_file(&url, &dest, sha, &|n| {
                progress(base + n.min(*size), MODEL_BYTES)
            }) {
                Ok(()) => {
                    errors.clear();
                    break;
                }
                Err(e) => errors.push(e),
            }
        }
        if !errors.is_empty() {
            return Err(format!("下载 {name} 失败：{}", errors.join("；")));
        }
        done += size;
        progress(done, MODEL_BYTES);
    }
    Ok(())
}

fn download_archive(
    library_root: &Path,
    dir: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let archive = library_root.join("models/speech/x-asr.tar.bz2");
    let mut errors = Vec::new();
    for url in ARCHIVE_URLS {
        match download_file(url, &archive, ARCHIVE_SHA256, &|n| {
            progress(
                n.min(ARCHIVE_BYTES) * MODEL_BYTES / ARCHIVE_BYTES,
                MODEL_BYTES,
            )
        }) {
            Ok(()) => {
                errors.clear();
                break;
            }
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("；"));
    }
    let file = File::open(&archive).map_err(|e| format!("无法打开模型整包：{e}"))?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(file));
    for entry in tar.entries().map_err(|e| format!("模型整包损坏：{e}"))? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry
            .path()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let Some((name, _, _)) = FILES
            .iter()
            .find(|(name, _, _)| path == *name || path.ends_with(&format!("/{name}")))
        else {
            continue;
        };
        entry
            .unpack(dir.join(name))
            .map_err(|e| format!("解压 {name} 失败：{e}"))?;
    }
    let _ = fs::remove_file(&archive);
    for (name, size, sha) in FILES {
        let path = dir.join(name);
        if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) != *size
            || !sha256_file(&path)?.eq_ignore_ascii_case(sha)
        {
            return Err(format!("解压后文件 {name} 校验失败"));
        }
    }
    progress(MODEL_BYTES, MODEL_BYTES);
    Ok(())
}

fn download_file(url: &str, dest: &Path, sha: &str, progress: &dyn Fn(u64)) -> Result<(), String> {
    let part = dest.with_extension(format!(
        "{}part",
        dest.extension()
            .and_then(|v| v.to_str())
            .map(|v| format!("{v}."))
            .unwrap_or_default()
    ));
    let existing = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(concat!(
            "RambleDesk/",
            env!("CARGO_PKG_VERSION"),
            " model-downloader"
        ))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
    if existing > 0 {
        request = request.header(RANGE, format!("bytes={existing}-"));
    }
    let mut response = request
        .send()
        .map_err(|e| format!("网络错误：{}", e.without_url()))?;
    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE && existing > 0 {
        // Let SHA validation decide whether this completed part is usable.
    } else if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let resumed = existing > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
    let mut out = if resumed {
        OpenOptions::new().append(true).open(&part)
    } else {
        File::create(&part)
    }
    .map_err(|e| e.to_string())?;
    let mut downloaded = if resumed { existing } else { 0 };
    progress(downloaded);
    if response.status() != StatusCode::RANGE_NOT_SATISFIABLE {
        let mut buf = vec![0; 256 * 1024];
        loop {
            let n = response
                .read(&mut buf)
                .map_err(|e| format!("下载中断：{e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            downloaded += n as u64;
            progress(downloaded);
        }
        out.flush().map_err(|e| e.to_string())?;
    }
    let actual = sha256_file(&part)?;
    if !actual.eq_ignore_ascii_case(sha) {
        let _ = fs::remove_file(&part);
        return Err(format!("SHA-256 校验失败（实际 {actual}）"));
    }
    if dest.exists() {
        fs::remove_file(dest).map_err(|e| e.to_string())?;
    }
    fs::rename(&part, dest).map_err(|e| format!("模型落盘失败：{e}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 256 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_size_matches_files() {
        assert_eq!(
            FILES.iter().map(|(_, size, _)| size).sum::<u64>(),
            MODEL_BYTES
        );
        assert!(FILES.iter().all(|(_, _, sha)| sha.len() == 64));
    }

    #[test]
    fn model_status_and_delete_are_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        assert!(!model_info(temp.path()).installed);
        let dir = model_dir(temp.path());
        fs::create_dir_all(&dir).unwrap();
        for (name, size, _) in FILES {
            File::create(dir.join(name))
                .unwrap()
                .set_len(*size)
                .unwrap();
        }
        assert!(model_info(temp.path()).installed);
        delete_model(temp.path()).unwrap();
        assert!(!dir.exists());
    }
}
