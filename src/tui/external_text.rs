use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    backend::MAX_EXTERNAL_TEXT_BYTES,
    product::config::TextInteractionHandlerConfig,
    runtime::{ApplicationGenerationId, OperationTicket},
    semantic::{BackendLocator, RuntimeNodeId},
    transcompile::InteractionScopeId,
};

use crate::runtime::artifacts::{OwnedArtifactDirectory, OwnedArtifactFile};

pub struct ExternalTextSession {
    pub(crate) target: RuntimeNodeId,
    pub(crate) locator: BackendLocator,
    pub(crate) generation: ApplicationGenerationId,
    pub(crate) scope: InteractionScopeId,
    pub(crate) original: String,
    pub(crate) ticket: OperationTicket,
    label: String,
    directory: Option<OwnedArtifactDirectory>,
    file: Option<OwnedArtifactFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandlerOutcome {
    Unchanged,
    Modified(String),
    Failed { reason: String, modified: bool },
}

impl ExternalTextSession {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        target: RuntimeNodeId,
        locator: BackendLocator,
        generation: ApplicationGenerationId,
        scope: InteractionScopeId,
        original: String,
        ticket: OperationTicket,
        label: String,
    ) -> Result<Self, String> {
        if original.len() > MAX_EXTERNAL_TEXT_BYTES {
            return Err("complete text exceeds the external interaction bound".into());
        }
        let mut directory =
            OwnedArtifactDirectory::new_owned(1800, ticket.session_id(), ticket.operation_id())
                .map_err(|_| "cannot create private text interaction directory".to_owned())?;
        let mut file = directory
            .create_file(".txt")
            .map_err(|_| "cannot create private text interaction file".to_owned())?;
        file.write_all(original.as_bytes())
            .and_then(|_| file.flush())
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "cannot initialize private text interaction file".to_owned())?;
        Ok(Self {
            target,
            locator,
            generation,
            scope,
            original,
            ticket,
            label,
            directory: Some(directory),
            file: Some(file),
        })
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn run_handler(&mut self, handler: &TextInteractionHandlerConfig) -> HandlerOutcome {
        let Some(file) = self.file.as_ref() else {
            return HandlerOutcome::Failed {
                reason: "private interaction representation is unavailable".into(),
                modified: false,
            };
        };
        let path = file.path().to_path_buf();
        let args = handler.args.iter().map(|argument| {
            if argument == "{file}" {
                path.as_os_str()
            } else {
                std::ffi::OsStr::new(argument)
            }
        });
        let status = match Command::new(&handler.program).args(args).status() {
            Ok(status) => status,
            Err(_) => {
                return HandlerOutcome::Failed {
                    reason: "configured text interaction handler could not be started".into(),
                    modified: false,
                };
            }
        };
        let candidate = match self.read_candidate() {
            Ok(candidate) => candidate,
            Err(reason) => {
                return HandlerOutcome::Failed {
                    reason,
                    modified: false,
                };
            }
        };
        let modified = candidate != self.original;
        if !status.success() {
            return HandlerOutcome::Failed {
                reason: "configured text interaction handler exited unsuccessfully".into(),
                modified,
            };
        }
        if modified {
            HandlerOutcome::Modified(candidate)
        } else {
            HandlerOutcome::Unchanged
        }
    }

    fn read_candidate(&mut self) -> Result<String, String> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| "private interaction representation is unavailable".to_owned())?;
        validate_original_path(file.path(), file.as_file())?;
        file.as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|_| "cannot read the handler result".to_owned())?;
        let mut bytes = Vec::new();
        file.as_file_mut()
            .take((MAX_EXTERNAL_TEXT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "cannot read the handler result".to_owned())?;
        if bytes.len() > MAX_EXTERNAL_TEXT_BYTES {
            return Err("handler result exceeds the external interaction bound".into());
        }
        String::from_utf8(bytes).map_err(|_| "handler result is not UTF-8 plain text".to_owned())
    }

    pub(crate) fn preserve(&mut self) -> Option<PathBuf> {
        let path = self.file.take()?.keep();
        let directory = self.directory.take()?.keep();
        path.starts_with(&directory).then_some(path)
    }
}

fn validate_original_path(path: &Path, open_file: &fs::File) -> Result<(), String> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|_| "handler removed the private interaction representation".to_owned())?;
    let open_metadata = open_file
        .metadata()
        .map_err(|_| "cannot validate the private interaction representation".to_owned())?;
    let current_uid = rustix::process::geteuid().as_raw();
    if !path_metadata.is_file()
        || path_metadata.uid() != current_uid
        || path_metadata.nlink() != 1
        || path_metadata.permissions().mode() & 0o077 != 0
        || path_metadata.dev() != open_metadata.dev()
        || path_metadata.ino() != open_metadata.ino()
    {
        return Err("handler replaced the private interaction representation unsafely".into());
    }
    Ok(())
}
