//! 编译用户选择的 `.proto` 源文件，并转换为动态 gRPC 驱动使用的 DescriptorSet。

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use protox::Compiler;
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
use ramag_domain::entities::{
    ApiGrpcDescriptor, MAX_API_DESCRIPTOR_BYTES, MAX_API_PROTO_SOURCE_BYTES,
    MAX_API_PROTO_SOURCE_TOTAL_BYTES,
};
use ramag_domain::error::DomainError;

/// 编译单个入口文件；导入文件限制在入口文件所在目录及其子目录内。
pub(crate) fn compile_descriptor_set(path: &Path) -> Result<Vec<u8>, String> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("proto"))
    {
        return Err("gRPC 源文件必须使用 .proto 后缀".into());
    }

    let root = path
        .parent()
        .ok_or_else(|| "无法确定 .proto 文件所在目录".to_string())?
        .canonicalize()
        .map_err(|_| "无法读取 .proto 文件所在目录".to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ".proto 文件名必须是有效的 UTF-8 文本".to_string())?;

    let total_source_bytes = Arc::new(AtomicUsize::new(0));
    let mut resolver = ChainFileResolver::new();
    resolver.add(BoundedIncludeResolver::new(root, total_source_bytes));
    resolver.add(GoogleFileResolver::new());

    let mut compiler = Compiler::with_file_resolver(resolver);
    compiler
        .include_imports(true)
        .include_source_info(false)
        .open_file(file_name)
        .map_err(|error| format!("编译 .proto 文件失败：{error}"))?;
    let bytes = compiler.encode_file_descriptor_set();
    if bytes.is_empty() {
        return Err("编译 .proto 文件没有生成 DescriptorSet".into());
    }
    if bytes.len() > MAX_API_DESCRIPTOR_BYTES {
        return Err(format!(
            "编译后的 DescriptorSet 超过 {MAX_API_DESCRIPTOR_BYTES} bytes 上限"
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Copy)]
pub(crate) enum GrpcImportKind {
    DescriptorSet,
    ProtoSource,
}

impl GrpcImportKind {
    pub(crate) fn success_message(self, bytes: usize) -> String {
        match self {
            Self::DescriptorSet => format!("已导入 FileDescriptorSet（{bytes} bytes）"),
            Self::ProtoSource => format!("已编译并导入 .proto（DescriptorSet {bytes} bytes）"),
        }
    }

    pub(crate) fn failure_message(self, error: String) -> String {
        match self {
            Self::DescriptorSet => format!("DescriptorSet 导入失败：{error}"),
            Self::ProtoSource => format!(".proto 编译失败：{error}"),
        }
    }
}

pub(crate) fn load_grpc_import(
    kind: GrpcImportKind,
    path: &Path,
) -> ramag_domain::error::Result<ApiGrpcDescriptor> {
    match kind {
        GrpcImportKind::DescriptorSet => {
            let file = fs::File::open(path).map_err(|error| {
                DomainError::Storage(format!("打开 DescriptorSet 文件失败：{error}"))
            })?;
            let metadata = file.metadata().map_err(|error| {
                DomainError::Storage(format!("读取 DescriptorSet 文件信息失败：{error}"))
            })?;
            let max_bytes = MAX_API_DESCRIPTOR_BYTES as u64;
            if !metadata.is_file() {
                return Err(DomainError::InvalidConfig(
                    "DescriptorSet 导入目标必须是普通文件".into(),
                ));
            }
            if metadata.len() > max_bytes {
                return Err(DomainError::InvalidConfig(format!(
                    "DescriptorSet 文件超过 {max_bytes} bytes 上限"
                )));
            }
            let mut bytes = Vec::new();
            file.take(max_bytes + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| {
                    DomainError::Storage(format!("读取 DescriptorSet 文件失败：{error}"))
                })?;
            if bytes.len() as u64 > max_bytes {
                return Err(DomainError::InvalidConfig(format!(
                    "DescriptorSet 文件读取后超过 {max_bytes} bytes 上限"
                )));
            }
            let descriptor = ApiGrpcDescriptor::FileDescriptorSet { bytes };
            descriptor.validate().map_err(DomainError::InvalidConfig)?;
            Ok(descriptor)
        }
        GrpcImportKind::ProtoSource => {
            let bytes = compile_descriptor_set(path).map_err(DomainError::InvalidConfig)?;
            let descriptor = ApiGrpcDescriptor::FileDescriptorSet { bytes };
            descriptor.validate().map_err(DomainError::InvalidConfig)?;
            Ok(descriptor)
        }
    }
}

struct BoundedIncludeResolver {
    root: PathBuf,
    total_source_bytes: Arc<AtomicUsize>,
}

impl BoundedIncludeResolver {
    fn new(root: PathBuf, total_source_bytes: Arc<AtomicUsize>) -> Self {
        Self {
            root,
            total_source_bytes,
        }
    }

    fn invalid_data(message: impl Into<String>) -> protox::Error {
        protox::Error::new(io::Error::new(io::ErrorKind::InvalidData, message.into()))
    }

    fn safe_relative_path(name: &str) -> Option<PathBuf> {
        let path = Path::new(name);
        if path.is_absolute() {
            return None;
        }
        if path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            Some(path.to_owned())
        } else {
            None
        }
    }

    fn reserve_source_bytes(&self, name: &str, bytes: usize) -> Result<(), protox::Error> {
        self.total_source_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes)
                    .filter(|next| *next <= MAX_API_PROTO_SOURCE_TOTAL_BYTES)
            })
            .map(|_| ())
            .map_err(|_| {
                Self::invalid_data(format!(
                    "导入的 .proto 源文件总量超过 {MAX_API_PROTO_SOURCE_TOTAL_BYTES} bytes 上限（{name}）"
                ))
            })
    }
}

impl FileResolver for BoundedIncludeResolver {
    fn resolve_path(&self, path: &Path) -> Option<String> {
        let canonical = path.canonicalize().ok()?;
        let relative = canonical.strip_prefix(&self.root).ok()?;
        let mut name = String::new();
        for component in relative.components() {
            let std::path::Component::Normal(component) = component else {
                return None;
            };
            if !name.is_empty() {
                name.push('/');
            }
            name.push_str(component.to_str()?);
        }
        (!name.is_empty()).then_some(name)
    }

    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        let Some(relative) = Self::safe_relative_path(name) else {
            return Err(Self::invalid_data(".proto 导入路径必须位于所选目录内"));
        };
        let path = self.root.join(relative);
        let canonical = path.canonicalize().map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => protox::Error::file_not_found(name),
            _ => protox::Error::new(error),
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(Self::invalid_data(".proto 导入路径不能离开所选目录"));
        }

        let metadata = fs::metadata(&canonical).map_err(protox::Error::new)?;
        if !metadata.is_file() {
            return Err(Self::invalid_data(format!(
                ".proto 导入目标不是普通文件：{name}"
            )));
        }
        if metadata.len() > MAX_API_PROTO_SOURCE_BYTES as u64 {
            return Err(Self::invalid_data(format!(
                ".proto 文件超过 {MAX_API_PROTO_SOURCE_BYTES} bytes 上限：{name}"
            )));
        }

        let file = fs::File::open(&canonical).map_err(protox::Error::new)?;
        let mut bytes = Vec::new();
        file.take(MAX_API_PROTO_SOURCE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(protox::Error::new)?;
        if bytes.len() > MAX_API_PROTO_SOURCE_BYTES {
            return Err(Self::invalid_data(format!(
                ".proto 文件读取后超过 {MAX_API_PROTO_SOURCE_BYTES} bytes 上限：{name}"
            )));
        }
        self.reserve_source_bytes(name, bytes.len())?;
        let source = String::from_utf8(bytes)
            .map_err(|_| Self::invalid_data(format!(".proto 文件必须使用 UTF-8 编码：{name}")))?;
        File::from_source(name, &source)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use protox::prost_reflect::DescriptorPool;

    use super::compile_descriptor_set;

    #[test]
    fn compiles_proto_with_relative_imports_into_dynamic_descriptor_set() {
        let directory = tempfile::tempdir().expect("创建临时目录");
        fs::write(
            directory.path().join("common.proto"),
            "syntax = \"proto3\"; package api.test; message EchoRequest { string message = 1; }",
        )
        .expect("写入依赖 proto");
        fs::write(
            directory.path().join("echo.proto"),
            "syntax = \"proto3\"; package api.test; import \"common.proto\"; message EchoResponse { string message = 1; } service Echo { rpc Unary(EchoRequest) returns (EchoResponse); }",
        )
        .expect("写入入口 proto");

        let bytes =
            compile_descriptor_set(&directory.path().join("echo.proto")).expect("proto 编译应成功");
        let pool = DescriptorPool::decode(bytes.as_slice()).expect("DescriptorSet 应可解码");
        let service = pool
            .get_service_by_name("api.test.Echo")
            .expect("Service 应存在");
        assert_eq!(service.methods().count(), 1);
        assert!(pool.get_message_by_name("api.test.EchoRequest").is_some());
    }

    #[test]
    fn rejects_proto_import_that_escapes_selected_directory() {
        let directory = tempfile::tempdir().expect("创建临时目录");
        fs::write(
            directory.path().join("outside.proto"),
            "syntax = \"proto3\"; message Outside {}",
        )
        .expect("写入外部 proto");
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).expect("创建入口目录");
        fs::write(
            nested.join("echo.proto"),
            "syntax = \"proto3\"; import \"../outside.proto\"; message Echo {}",
        )
        .expect("写入入口 proto");

        let result = compile_descriptor_set(&nested.join("echo.proto"));
        assert!(result.is_err());
    }
}
