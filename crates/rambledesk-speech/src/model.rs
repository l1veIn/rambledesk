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

pub const X_ASR_MODEL_ID: &str = "x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05";
pub const SENSEVOICE_MODEL_ID: &str = "sense-voice-zh-en-ja-ko-yue-2024-07-17";
pub const FUNASR_NANO_MODEL_ID: &str = "funasr-nano-int8-2025-12-30";
pub const DEFAULT_MODEL_ID: &str = SENSEVOICE_MODEL_ID;

const X_ASR_ENGINE_ID: &str = "sherpa-onnx-x-asr-zh-en";
const SENSEVOICE_ENGINE_ID: &str = "sherpa-onnx-sensevoice";
const FUNASR_NANO_ENGINE_ID: &str = "sherpa-onnx-funasr-nano";

const X_ASR_FILES: &[ModelFile] = &[
    ModelFile::sha(
        "encoder.int8.onnx",
        "",
        155_278_641,
        "908596dcc137a73b95be908ca55e88caa1b3dbbe8027c171615f4b0609c5eb1e",
    ),
    ModelFile::sha(
        "decoder.onnx",
        "",
        11_309_084,
        "a1cbc9eac2d5e3fb6617a218c67ad6daaa7f4e0fd225f08b2c22ab0413c8c257",
    ),
    ModelFile::sha(
        "joiner.int8.onnx",
        "",
        2_581_422,
        "aedb7fa697b2ab43f20499826fff7c997eea7d67db77be97769aeeeb726e63b3",
    ),
    ModelFile::sha(
        "tokens.txt",
        "",
        58_806,
        "b818a60878b9aae978cbb8ad594acbd403d76d1af2e31ef4197c84e2dbdba27c",
    ),
    ModelFile::sha(
        "bpe.model",
        "",
        119_265,
        "f87a38025a5fdd1e4e9591f6a44bb81295097ce0b80df6f4ab9f44e52c64ca5f",
    ),
];

const SENSEVOICE_FILES: &[ModelFile] = &[
    ModelFile::sha(
        "model.int8.onnx",
        "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx",
        239_233_841,
        "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51",
    ),
    ModelFile::size_only(
        "tokens.txt",
        "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt",
        315_894,
    ),
];

const FUNASR_NANO_FILES: &[ModelFile] = &[
    ModelFile::sha(
        "encoder_adaptor.int8.onnx",
        "https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30/resolve/main/encoder_adaptor.int8.onnx",
        237_792_748,
        "f36dea2e30fbc33b5db1d7a7265cc976c5e5586c77b042d5adb1ad27c72db422",
    ),
    ModelFile::sha(
        "llm.int8.onnx",
        "https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30/resolve/main/llm.int8.onnx",
        600_356_593,
        "dfbf9aa3be41bccc257587f151e15c63fbe1b549f2b517f5ccd5bdce3bf4322a",
    ),
    ModelFile::sha(
        "embedding.int8.onnx",
        "https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30/resolve/main/embedding.int8.onnx",
        155_584_380,
        "95e61cd0c9c3b9543339a4cf973c95c116815e745ccc1e0285cbd81f76d18644",
    ),
    ModelFile::size_only(
        "Qwen3-0.6B/merges.txt",
        "https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30/resolve/main/Qwen3-0.6B/merges.txt",
        1_671_853,
    ),
    ModelFile::sha(
        "Qwen3-0.6B/tokenizer.json",
        "https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30/resolve/main/Qwen3-0.6B/tokenizer.json",
        11_422_654,
        "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
    ),
    ModelFile::sha(
        "Qwen3-0.6B/vocab.json",
        "https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30/resolve/main/Qwen3-0.6B/vocab.json",
        2_776_833,
        "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
    ),
];

const X_ASR_ARCHIVE: ArchiveSource = ArchiveSource {
    bytes: 133_895_136,
    sha256: "fa5f63d618e5a01526e275a358bb7772e403f84808a4769fba52cffd8160bf74",
    urls: &[
        "https://ghfast.top/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2",
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2",
    ],
};

const X_ASR_MIRRORS: &[FileMirror] = &[
    FileMirror {
        base_url: "https://www.modelscope.cn/models/yangchen1258/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05",
        revision: "aa50faf0a9e45f6ea8913762151d47679ba468d7",
    },
    FileMirror {
        base_url: "https://www.modelscope.cn/models/bujidc/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05",
        revision: "57f0a27e56d43b36f350be2ecc4a2200232d13e7",
    },
];

const MODELS: &[ModelManifest] = &[
    ModelManifest {
        id: SENSEVOICE_MODEL_ID,
        engine_id: SENSEVOICE_ENGINE_ID,
        display_name: "SenseVoice 多语言",
        description: "推荐默认；VAD 自动分段后整段识别，兼顾多语言准确率与稳定性",
        directory: "sherpa-sensevoice",
        languages: &["中文", "English", "日本語", "한국어", "粤语"],
        license: "FunASR Model License 1.1",
        streaming: false,
        hotwords_supported: false,
        files: SENSEVOICE_FILES,
        archive: None,
        mirrors: &[],
    },
    ModelManifest {
        id: X_ASR_MODEL_ID,
        engine_id: X_ASR_ENGINE_ID,
        display_name: "X-ASR 流式中英标点",
        description: "低延迟实时出字，适合需要流式反馈的持续 Ramble",
        directory: "sherpa-x-asr",
        languages: &["中文", "English"],
        license: "Apache-2.0",
        streaming: true,
        hotwords_supported: true,
        files: X_ASR_FILES,
        archive: Some(X_ASR_ARCHIVE),
        mirrors: X_ASR_MIRRORS,
    },
    ModelManifest {
        id: FUNASR_NANO_MODEL_ID,
        engine_id: FUNASR_NANO_ENGINE_ID,
        display_name: "FunASR-Nano 中英日",
        description: "VAD 自动分段的高质量非流式模型，下载和内存占用较大",
        directory: "sherpa-funasr-nano",
        languages: &["中文", "English", "日本語"],
        license: "FunASR Model License",
        streaming: false,
        hotwords_supported: true,
        files: FUNASR_NANO_FILES,
        archive: None,
        mirrors: &[],
    },
];

#[derive(Clone, Copy)]
struct ModelFile {
    name: &'static str,
    url: &'static str,
    bytes: u64,
    sha256: Option<&'static str>,
}

impl ModelFile {
    const fn sha(name: &'static str, url: &'static str, bytes: u64, sha256: &'static str) -> Self {
        Self {
            name,
            url,
            bytes,
            sha256: Some(sha256),
        }
    }

    const fn size_only(name: &'static str, url: &'static str, bytes: u64) -> Self {
        Self {
            name,
            url,
            bytes,
            sha256: None,
        }
    }
}

#[derive(Clone, Copy)]
struct ArchiveSource {
    bytes: u64,
    sha256: &'static str,
    urls: &'static [&'static str],
}

struct FileMirror {
    base_url: &'static str,
    revision: &'static str,
}

struct ModelManifest {
    id: &'static str,
    engine_id: &'static str,
    display_name: &'static str,
    description: &'static str,
    directory: &'static str,
    languages: &'static [&'static str],
    license: &'static str,
    streaming: bool,
    hotwords_supported: bool,
    files: &'static [ModelFile],
    archive: Option<ArchiveSource>,
    mirrors: &'static [FileMirror],
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeechModelInfo {
    pub id: &'static str,
    pub engine_id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub size_bytes: u64,
    pub installed: bool,
    pub path: String,
    pub missing_files: Vec<String>,
    pub streaming: bool,
    pub hotwords_supported: bool,
    pub languages: &'static [&'static str],
    pub license: &'static str,
}

pub fn list_models(library_root: &Path) -> Vec<SpeechModelInfo> {
    MODELS
        .iter()
        .map(|model| model_info_from_manifest(library_root, model))
        .collect()
}

pub fn model_info(library_root: &Path, model_id: &str) -> Result<SpeechModelInfo, String> {
    let model = manifest(model_id)?;
    Ok(model_info_from_manifest(library_root, model))
}

pub fn model_dir(library_root: &Path, model_id: &str) -> Result<PathBuf, String> {
    Ok(library_root
        .join("models")
        .join("speech")
        .join(manifest(model_id)?.directory))
}

pub fn model_engine_id(model_id: &str) -> Result<&'static str, String> {
    Ok(manifest(model_id)?.engine_id)
}

fn manifest(model_id: &str) -> Result<&'static ModelManifest, String> {
    MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| format!("未知语音模型：{model_id}"))
}

fn model_info_from_manifest(library_root: &Path, model: &'static ModelManifest) -> SpeechModelInfo {
    let dir = library_root
        .join("models")
        .join("speech")
        .join(model.directory);
    let missing_files = model
        .files
        .iter()
        .filter_map(|file| match fs::metadata(dir.join(file.name)) {
            Ok(metadata) if metadata.len() == file.bytes => None,
            Ok(metadata) => Some(format!(
                "{}（大小不符：期望 {}，实际 {}）",
                file.name,
                file.bytes,
                metadata.len()
            )),
            Err(_) => Some(format!("{}（缺失）", file.name)),
        })
        .collect::<Vec<_>>();
    SpeechModelInfo {
        id: model.id,
        engine_id: model.engine_id,
        display_name: model.display_name,
        description: model.description,
        size_bytes: model.files.iter().map(|file| file.bytes).sum(),
        installed: missing_files.is_empty(),
        path: display_path(&dir),
        missing_files,
        streaming: model.streaming,
        hotwords_supported: model.hotwords_supported,
        languages: model.languages,
        license: model.license,
    }
}

/// Whether a model's recognizer accepts contextual hotwords. SenseVoice does
/// not; X-ASR (online transducer) and FunASR-Nano do.
pub fn model_supports_hotwords(model_id: &str) -> Result<bool, String> {
    Ok(manifest(model_id)?.hotwords_supported)
}

pub fn delete_model(library_root: &Path, model_id: &str) -> Result<(), String> {
    let dir = model_dir(library_root, model_id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|error| format!("删除模型 {} 失败：{error}", dir.display()))?;
    }
    Ok(())
}

pub fn download_model(
    library_root: &Path,
    model_id: &str,
    progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let model = manifest(model_id)?;
    let dir = model_dir(library_root, model_id)?;
    fs::create_dir_all(&dir).map_err(|error| format!("无法创建模型目录：{error}"))?;
    let total = model.files.iter().map(|file| file.bytes).sum::<u64>();
    if model_info_from_manifest(library_root, model).installed {
        progress(total, total);
        return Ok(());
    }

    let result = if !model.mirrors.is_empty() {
        let mirror_error = download_from_mirrors(model, &dir, progress).err();
        if model_info_from_manifest(library_root, model).installed {
            Ok(())
        } else if let Some(archive) = model.archive {
            download_archive(library_root, model, archive, &dir, progress).map_err(|official| {
                match mirror_error {
                    Some(mirror) => {
                        format!("ModelScope 镜像失败：{mirror}；GitHub 源也失败：{official}")
                    }
                    None => official,
                }
            })
        } else {
            Err(mirror_error.unwrap_or_else(|| "没有可用的模型下载源".to_owned()))
        }
    } else {
        download_manifest_files(model, &dir, progress)
    };

    result?;
    if !model_info_from_manifest(library_root, model).installed {
        return Err("模型下载完成，但完整性检查未通过".into());
    }
    progress(total, total);
    Ok(())
}

fn download_manifest_files(
    model: &ModelManifest,
    dir: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let total = model.files.iter().map(|file| file.bytes).sum::<u64>();
    let mut done = 0;
    for file in model.files {
        let destination = dir.join(file.name);
        if fs::metadata(&destination).is_ok_and(|metadata| metadata.len() == file.bytes) {
            done += file.bytes;
            progress(done, total);
            continue;
        }
        let base = done;
        download_file(
            file.url,
            &destination,
            file.bytes,
            file.sha256,
            &|downloaded| progress(base + downloaded.min(file.bytes), total),
        )?;
        done += file.bytes;
        progress(done, total);
    }
    Ok(())
}

fn download_from_mirrors(
    model: &ModelManifest,
    dir: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let total = model.files.iter().map(|file| file.bytes).sum::<u64>();
    let mut done = 0;
    for file in model.files {
        let destination = dir.join(file.name);
        if fs::metadata(&destination).is_ok_and(|metadata| metadata.len() == file.bytes) {
            done += file.bytes;
            progress(done, total);
            continue;
        }
        let base = done;
        let mut errors = Vec::new();
        for mirror in model.mirrors {
            let url = format!(
                "{}/resolve/{}/{}",
                mirror.base_url.trim_end_matches('/'),
                mirror.revision,
                file.name
            );
            match download_file(&url, &destination, file.bytes, file.sha256, &|downloaded| {
                progress(base + downloaded.min(file.bytes), total)
            }) {
                Ok(()) => {
                    errors.clear();
                    break;
                }
                Err(error) => errors.push(error),
            }
        }
        if !errors.is_empty() {
            return Err(format!("下载 {} 失败：{}", file.name, errors.join("；")));
        }
        done += file.bytes;
        progress(done, total);
    }
    Ok(())
}

fn download_archive(
    library_root: &Path,
    model: &ModelManifest,
    archive: ArchiveSource,
    dir: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let total = model.files.iter().map(|file| file.bytes).sum::<u64>();
    let archive_path = library_root
        .join("models")
        .join("speech")
        .join(format!("{}.tar.bz2", model.directory));
    let mut errors = Vec::new();
    for url in archive.urls {
        match download_file(
            url,
            &archive_path,
            archive.bytes,
            Some(archive.sha256),
            &|downloaded| progress(downloaded.min(archive.bytes) * total / archive.bytes, total),
        ) {
            Ok(()) => {
                errors.clear();
                break;
            }
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("；"));
    }

    let file = File::open(&archive_path).map_err(|error| format!("无法打开模型整包：{error}"))?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(file));
    for entry in tar
        .entries()
        .map_err(|error| format!("模型整包损坏：{error}"))?
    {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let path = entry
            .path()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let Some(model_file) = model
            .files
            .iter()
            .find(|file| path == file.name || path.ends_with(&format!("/{}", file.name)))
        else {
            continue;
        };
        let destination = dir.join(model_file.name);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        entry
            .unpack(&destination)
            .map_err(|error| format!("解压 {} 失败：{error}", model_file.name))?;
    }
    let _ = fs::remove_file(&archive_path);
    validate_model_files(model, dir)?;
    Ok(())
}

fn validate_model_files(model: &ModelManifest, dir: &Path) -> Result<(), String> {
    for file in model.files {
        let path = dir.join(file.name);
        let actual_size = fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if actual_size != file.bytes {
            return Err(format!("文件 {} 大小校验失败", file.name));
        }
        if let Some(expected) = file.sha256
            && !sha256_file(&path)?.eq_ignore_ascii_case(expected)
        {
            return Err(format!("文件 {} SHA-256 校验失败", file.name));
        }
    }
    Ok(())
}

fn download_file(
    url: &str,
    destination: &Path,
    expected_bytes: u64,
    expected_sha256: Option<&str>,
    progress: &dyn Fn(u64),
) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let part = destination.with_extension(format!(
        "{}part",
        destination
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!("{value}."))
            .unwrap_or_default()
    ));
    let existing = fs::metadata(&part)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(concat!(
            "RambleDesk/",
            env!("CARGO_PKG_VERSION"),
            " model-downloader"
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
    if existing > 0 {
        request = request.header(RANGE, format!("bytes={existing}-"));
    }
    let mut response = request
        .send()
        .map_err(|error| format!("网络错误：{}", error.without_url()))?;
    if response.status() != StatusCode::RANGE_NOT_SATISFIABLE && !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let resumed = existing > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
    let mut output = if resumed {
        OpenOptions::new().append(true).open(&part)
    } else {
        File::create(&part)
    }
    .map_err(|error| error.to_string())?;
    let mut downloaded = if resumed { existing } else { 0 };
    progress(downloaded);
    if response.status() != StatusCode::RANGE_NOT_SATISFIABLE {
        let mut buffer = vec![0; 256 * 1024];
        loop {
            let count = response
                .read(&mut buffer)
                .map_err(|error| format!("下载中断：{error}"))?;
            if count == 0 {
                break;
            }
            output
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            downloaded += count as u64;
            progress(downloaded);
        }
        output.flush().map_err(|error| error.to_string())?;
    }
    let actual_bytes = fs::metadata(&part)
        .map_err(|error| error.to_string())?
        .len();
    if actual_bytes != expected_bytes {
        let _ = fs::remove_file(&part);
        return Err(format!(
            "文件大小校验失败（期望 {expected_bytes}，实际 {actual_bytes}）"
        ));
    }
    if let Some(expected) = expected_sha256 {
        let actual = sha256_file(&part)?;
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = fs::remove_file(&part);
            return Err(format!("SHA-256 校验失败（实际 {actual}）"));
        }
    }
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| error.to_string())?;
    }
    fs::rename(&part, destination).map_err(|error| format!("模型落盘失败：{error}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotword_capability_matches_manifest() {
        assert!(model_supports_hotwords(X_ASR_MODEL_ID).unwrap());
        assert!(!model_supports_hotwords(SENSEVOICE_MODEL_ID).unwrap());
        assert!(model_supports_hotwords(FUNASR_NANO_MODEL_ID).unwrap());
        assert!(model_supports_hotwords("unknown-model").is_err());
    }

    #[test]
    fn manifest_has_three_supported_models_with_unique_ids() {
        assert_eq!(MODELS.len(), 3);
        assert_eq!(DEFAULT_MODEL_ID, SENSEVOICE_MODEL_ID);
        assert_eq!(MODELS[0].id, DEFAULT_MODEL_ID);
        assert_eq!(MODELS[1].id, X_ASR_MODEL_ID);
        let mut ids = MODELS.iter().map(|model| model.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MODELS.len());
        assert!(MODELS.iter().any(|model| model.streaming));
        assert_eq!(MODELS.iter().filter(|model| !model.streaming).count(), 2);
    }

    #[test]
    fn manifest_sizes_and_hashes_are_well_formed() {
        for model in MODELS {
            assert!(!model.files.is_empty());
            for file in model.files {
                assert!(file.bytes > 0);
                assert!(!file.name.contains(".."));
                if let Some(sha) = file.sha256 {
                    assert_eq!(sha.len(), 64);
                    assert!(sha.chars().all(|character| character.is_ascii_hexdigit()));
                }
            }
        }
    }

    #[test]
    fn model_status_and_delete_are_scoped_by_model_id() {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            list_models(temp.path())
                .iter()
                .all(|model| !model.installed)
        );
        let manifest = manifest(SENSEVOICE_MODEL_ID).unwrap();
        let dir = model_dir(temp.path(), SENSEVOICE_MODEL_ID).unwrap();
        fs::create_dir_all(&dir).unwrap();
        for file in manifest.files {
            let path = dir.join(file.name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            File::create(path).unwrap().set_len(file.bytes).unwrap();
        }
        assert!(
            model_info(temp.path(), SENSEVOICE_MODEL_ID)
                .unwrap()
                .installed
        );
        assert!(!model_info(temp.path(), X_ASR_MODEL_ID).unwrap().installed);
        delete_model(temp.path(), SENSEVOICE_MODEL_ID).unwrap();
        assert!(!dir.exists());
    }
}
