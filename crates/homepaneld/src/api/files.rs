use crate::state::AppState;
use axum::{
    extract::{Multipart, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use homepanel_core::{error::{ApiError, ApiErrorResponse}, types::FileAllowlist};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const PREVIEW_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NameRequest {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub path: String,
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified: Option<String>,
    pub readonly: bool,
}

#[derive(Debug, Serialize)]
pub struct FileListResponse {
    pub path: String,
    pub parent_path: Option<String>,
    pub allowed_roots: Vec<String>,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
pub struct FilePreviewResponse {
    pub path: String,
    pub size: u64,
    pub truncated: bool,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct FileActionResponse {
    pub ok: bool,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct FileDeleteResponse {
    pub ok: bool,
    pub parent_path: Option<String>,
}

fn allowlist(state: &AppState) -> FileAllowlist {
    FileAllowlist {
        allowed_paths: state.config.files.allowed_paths.clone(),
    }
}

fn api_error_response(err: ApiError) -> Response {
    let status = match &err {
        ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
        ApiError::Forbidden => StatusCode::FORBIDDEN,
        ApiError::NotFound => StatusCode::NOT_FOUND,
        ApiError::BadRequest(_) | ApiError::Message { .. } | ApiError::Json(_) => {
            StatusCode::BAD_REQUEST
        }
        ApiError::Io(io) => match io.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            std::io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
            std::io::ErrorKind::AlreadyExists => StatusCode::CONFLICT,
            std::io::ErrorKind::DirectoryNotEmpty => StatusCode::CONFLICT,
            _ => StatusCode::BAD_REQUEST,
        },
        ApiError::Config(_) | ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(ApiErrorResponse::from(err))).into_response()
}

fn bad_request(message: impl Into<String>) -> Response {
    api_error_response(ApiError::bad_request(message))
}

fn forbidden() -> Response {
    api_error_response(ApiError::Forbidden)
}

fn resolved_allowed_roots(state: &AppState) -> Vec<PathBuf> {
    state
        .config
        .files
        .allowed_paths
        .iter()
        .filter_map(|root| {
            let canonical = fs::canonicalize(root).ok()?;
            let metadata = fs::metadata(&canonical).ok()?;
            if !metadata.is_dir() || fs::read_dir(&canonical).is_err() {
                return None;
            }
            Some(canonical)
        })
        .collect()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn validate_component_name(name: &str) -> Result<&str, ApiError> {
    if name.is_empty() || name.contains('\0') {
        return Err(ApiError::bad_request("name is required"));
    }

    let mut components = Path::new(name).components();
    if components.next().is_none() || components.next().is_some() {
        return Err(ApiError::bad_request("name must not contain path separators"));
    }

    if name == "." || name == ".." {
        return Err(ApiError::bad_request("name must not be a traversal segment"));
    }

    Ok(name)
}

fn default_root(state: &AppState) -> Result<PathBuf, ApiError> {
    resolved_allowed_roots(state)
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::bad_request("no accessible allowed paths are configured"))
}

fn ensure_absolute(path: &Path) -> Result<(), ApiError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ApiError::bad_request("path must be absolute"))
    }
}

fn canonicalize_existing_path(state: &AppState, raw_path: &str) -> Result<PathBuf, ApiError> {
    if raw_path.contains('\0') {
        return Err(ApiError::bad_request("path contains an invalid byte"));
    }

    let path = PathBuf::from(raw_path);
    ensure_absolute(&path)?;

    let canonical = fs::canonicalize(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => ApiError::NotFound,
        std::io::ErrorKind::PermissionDenied => ApiError::Forbidden,
        _ => ApiError::Io(err),
    })?;

    if allowlist(state).is_allowed(&canonical) {
        Ok(canonical)
    } else {
        Err(ApiError::Forbidden)
    }
}

fn resolve_requested_directory(
    state: &AppState,
    raw_path: Option<&str>,
) -> Result<PathBuf, ApiError> {
    match raw_path {
        Some(path) if !path.trim().is_empty() => canonicalize_existing_path(state, path),
        _ => canonicalize_existing_path(state, &path_to_string(&default_root(state)?)),
    }
}

fn is_within_allowed_roots(path: &Path, allowed_roots: &[PathBuf]) -> bool {
    allowed_roots.iter().any(|root| {
        path == root
            || path
                .strip_prefix(root)
                .map(|rest| !rest.components().any(|component| {
                    matches!(component, std::path::Component::ParentDir)
                }))
                .unwrap_or(false)
    })
}

fn resolve_parent_if_allowed(path: &Path, allowed_roots: &[PathBuf]) -> Option<String> {
    let parent = path.parent()?;
    if is_within_allowed_roots(parent, allowed_roots) {
        Some(path_to_string(parent))
    } else {
        None
    }
}

fn file_kind(file_type: &fs::FileType) -> FileKind {
    if file_type.is_symlink() {
        FileKind::Symlink
    } else if file_type.is_dir() {
        FileKind::Dir
    } else if file_type.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    }
}

fn file_entry(entry: fs::DirEntry) -> Result<FileEntry, ApiError> {
    let path = entry.path();
    let metadata = fs::symlink_metadata(&path).map_err(ApiError::Io)?;
    let file_type = metadata.file_type();
    let modified = metadata
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .map(|dt| dt.to_rfc3339());
    let name = entry.file_name().to_string_lossy().into_owned();

    Ok(FileEntry {
        name,
        path: path_to_string(&path),
        kind: file_kind(&file_type),
        size: metadata.len(),
        modified,
        readonly: metadata.permissions().readonly(),
    })
}

fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| {
        let rank = |kind: &FileKind| match kind {
            FileKind::Dir => 0,
            FileKind::Symlink => 1,
            FileKind::File => 2,
            FileKind::Other => 3,
        };

        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

fn is_probably_text(path: &Path, bytes: &[u8]) -> bool {
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return false;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    matches!(
        extension.as_str(),
        "txt"
            | "md"
            | "rst"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "ini"
            | "conf"
            | "cfg"
            | "log"
            | "csv"
            | "xml"
            | "html"
            | "htm"
            | "css"
            | "js"
            | "mjs"
            | "ts"
            | "tsx"
            | "jsx"
            | "rs"
            | "py"
            | "sh"
            | "service"
            | "timer"
            | "socket"
            | "target"
            | "mount"
            | "path"
            | "slice"
    ) || matches!(
        file_name.as_str(),
        "makefile" | "dockerfile" | "readme" | "readme.md" | "license"
    )
}

fn preview_content(path: &Path) -> Result<FilePreviewResponse, ApiError> {
    let metadata = fs::metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => ApiError::NotFound,
        std::io::ErrorKind::PermissionDenied => ApiError::Forbidden,
        _ => ApiError::Io(err),
    })?;

    if !metadata.is_file() {
        return Err(ApiError::bad_request("preview is only available for files"));
    }

    let file = fs::File::open(path).map_err(ApiError::Io)?;
    let mut bytes = Vec::new();
    file.take(PREVIEW_LIMIT_BYTES as u64)
        .read_to_end(&mut bytes)
        .map_err(ApiError::Io)?;

    if !is_probably_text(path, &bytes) {
        return Err(ApiError::bad_request("preview is only available for text-like files"));
    }

    let content = String::from_utf8(bytes).map_err(|_| {
        ApiError::bad_request("preview is only available for text-like files")
    })?;

    Ok(FilePreviewResponse {
        path: path_to_string(path),
        size: metadata.len(),
        truncated: metadata.len() as usize > PREVIEW_LIMIT_BYTES,
        content,
    })
}

fn directory_list_response(state: &AppState, directory: &Path) -> Result<FileListResponse, ApiError> {
    let allowed_roots = resolved_allowed_roots(state);
    if !is_within_allowed_roots(directory, &allowed_roots) {
        return Err(ApiError::Forbidden);
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(directory).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => ApiError::NotFound,
        std::io::ErrorKind::PermissionDenied => ApiError::Forbidden,
        _ => ApiError::Io(err),
    })? {
        let entry = entry.map_err(ApiError::Io)?;
        if let Ok(file_entry) = file_entry(entry) {
            entries.push(file_entry);
        }
    }
    sort_entries(&mut entries);

    Ok(FileListResponse {
        path: path_to_string(directory),
        parent_path: resolve_parent_if_allowed(directory, &allowed_roots),
        allowed_roots: allowed_roots.iter().map(|path| path_to_string(path)).collect(),
        entries,
    })
}

fn build_target_path(base: &Path, name: &str) -> Result<PathBuf, ApiError> {
    let name = validate_component_name(name)?;
    Ok(base.join(name))
}

fn ensure_parent_allowed(state: &AppState, path: &Path) -> Result<PathBuf, ApiError> {
    let allowed_roots = resolved_allowed_roots(state);
    if is_within_allowed_roots(path, &allowed_roots) {
        Ok(path.to_path_buf())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub async fn list(State(state): State<AppState>, Query(query): Query<PathQuery>) -> impl IntoResponse {
    match resolve_requested_directory(&state, query.path.as_deref()) {
        Ok(path) => match directory_list_response(&state, &path) {
        Ok(response) => Json(response).into_response(),
        Err(err) => api_error_response(err),
    },
        Err(err) => api_error_response(err),
    }
}

pub async fn preview(State(state): State<AppState>, Query(query): Query<PathQuery>) -> impl IntoResponse {
    let Some(path) = query.path.as_deref() else {
        return bad_request("path is required");
    };
    match canonicalize_existing_path(&state, path) {
        Ok(path) => match preview_content(&path) {
            Ok(response) => Json(response).into_response(),
            Err(err) => api_error_response(err),
        },
        Err(err) => api_error_response(err),
    }
}

pub async fn mkdir(
    State(state): State<AppState>,
    Json(body): Json<NameRequest>,
) -> impl IntoResponse {
    let parent = match canonicalize_existing_path(&state, &body.path) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };
    let target = match build_target_path(&parent, &body.name) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };

    if !ensure_parent_allowed(&state, &parent).is_ok() {
        return forbidden();
    }

    match fs::create_dir(&target) {
        Ok(()) => Json(FileActionResponse {
            ok: true,
            path: path_to_string(&target),
        })
        .into_response(),
        Err(err) => api_error_response(ApiError::Io(err)),
    }
}

pub async fn rename(
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> impl IntoResponse {
    let source = match canonicalize_existing_path(&state, &body.path) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };

    let parent = match source.parent() {
        Some(parent) => parent.to_path_buf(),
        None => return bad_request("cannot rename this path"),
    };
    let allowed_roots = resolved_allowed_roots(&state);
    if allowed_roots.iter().any(|root| root == &source) {
        return bad_request("cannot rename an allowed root");
    }
    if !ensure_parent_allowed(&state, &source).is_ok() {
        return forbidden();
    }

    let target = match build_target_path(&parent, &body.new_name) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };

    match fs::rename(&source, &target) {
        Ok(()) => Json(FileActionResponse {
            ok: true,
            path: path_to_string(&target),
        })
        .into_response(),
        Err(err) => api_error_response(ApiError::Io(err)),
    }
}

pub async fn delete(State(state): State<AppState>, Json(body): Json<DeleteRequest>) -> impl IntoResponse {
    let path = match canonicalize_existing_path(&state, &body.path) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };

    let allowed_roots = resolved_allowed_roots(&state);
    if allowed_roots.iter().any(|root| root == &path) {
        return bad_request("cannot delete an allowed root");
    }

    let parent_path = resolve_parent_if_allowed(&path, &allowed_roots);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) => return api_error_response(ApiError::Io(err)),
    };

    let result = if metadata.file_type().is_dir() {
        fs::remove_dir(&path)
    } else {
        fs::remove_file(&path)
    };

    match result {
        Ok(()) => Json(FileDeleteResponse {
            ok: true,
            parent_path,
        })
        .into_response(),
        Err(err) => api_error_response(ApiError::Io(err)),
    }
}

pub async fn upload(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let directory = match resolve_requested_directory(&state, query.path.as_deref()) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };

    let allowed_roots = resolved_allowed_roots(&state);
    if !is_within_allowed_roots(&directory, &allowed_roots) {
        return forbidden();
    }

    let mut uploaded_path: Option<PathBuf> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        let Some(filename) = field.file_name().map(ToOwned::to_owned) else {
            continue;
        };
        let target = match build_target_path(&directory, &filename) {
            Ok(path) => path,
            Err(err) => return api_error_response(err),
        };
        let bytes = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(err) => return api_error_response(ApiError::BadRequest(err.to_string())),
        };
        if let Err(err) = tokio::fs::write(&target, &bytes).await {
            return api_error_response(ApiError::Io(err));
        }
        uploaded_path = Some(target);
        break;
    }

    match uploaded_path {
        Some(path) => Json(FileActionResponse {
            ok: true,
            path: path_to_string(&path),
        })
        .into_response(),
        None => bad_request("upload is missing a file field"),
    }
}

pub async fn download(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> impl IntoResponse {
    let Some(path) = query.path.as_deref() else {
        return bad_request("path is required");
    };
    let path = match canonicalize_existing_path(&state, path) {
        Ok(path) => path,
        Err(err) => return api_error_response(err),
    };
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) => return api_error_response(ApiError::Io(err)),
    };
    if !metadata.is_file() {
        return bad_request("download is only available for files");
    }

    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => return api_error_response(ApiError::Io(err)),
    };
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download.bin")
        .replace('"', "_");

    (
        [
            (CONTENT_TYPE, HeaderValue::from_static("application/octet-stream")),
            (
                CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                    .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
            ),
        ],
        bytes,
    )
        .into_response()
}
