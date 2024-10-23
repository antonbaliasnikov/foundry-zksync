use crate::provider::VerificationContext;

use alloy_json_abi::JsonAbi;
use eyre::{OptionExt, Result};
use foundry_common::compile::ProjectCompiler;
use foundry_compilers::{
    artifacts::{output_selection::OutputSelection, Source},
    compilers::CompilerSettings,
    resolver::parse::SolData,
    solc::{Solc, SolcCompiler},
    zksolc::{self, ZkSolc, ZkSolcCompiler},
    zksync::artifact_output::zk::ZkArtifactOutput,
    Graph, Project,
};
use foundry_config::Config;
use semver::Version;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ZkVersion {
    pub zksolc: Version,
    pub solc: Version,
    pub is_zksync_solc: bool,
}

/// Container with data required for contract verification.
#[derive(Debug, Clone)]
pub struct ZkVerificationContext {
    pub config: Config,
    pub project: Project<ZkSolcCompiler, ZkArtifactOutput>,
    pub target_path: PathBuf,
    pub target_name: String,
    pub compiler_version: ZkVersion,
}

impl ZkVerificationContext {
    pub fn new(
        target_path: PathBuf,
        target_name: String,
        context_solc_version: Version,
        config: Config,
    ) -> Result<Self> {
        let mut project =
            foundry_zksync_compiler::config_create_project(&config, config.cache, false)?;
        project.no_artifacts = true;
        let zksolc_version = ZkSolc::get_version_for_path(&project.compiler.zksolc)?;

        let (solc_version, is_zksync_solc) = if let Some(solc) = &config.zksync.solc_path {
            let solc_type_and_version = zksolc::get_solc_version_info(solc)?;
            (solc_type_and_version.version, solc_type_and_version.zksync_version.is_some())
        } else {
            //if there's no `solc_path` specified then we use the same
            // as the project version
            let maybe_solc_path =
                ZkSolc::find_solc_installed_version(&context_solc_version.to_string())?;
            let solc_path = if let Some(p) = maybe_solc_path {
                p
            } else {
                ZkSolc::solc_blocking_install(&context_solc_version.to_string())?
            };

            let solc = Solc::new_with_version(solc_path, context_solc_version.clone());
            project.compiler.solc = SolcCompiler::Specific(solc);

            (context_solc_version, true)
        };

        let compiler_version =
            ZkVersion { zksolc: zksolc_version, solc: solc_version, is_zksync_solc };

        Ok(Self { config, project, target_name, target_path, compiler_version })
    }

    /// Compiles target contract requesting only ABI and returns it.
    pub fn get_target_abi(&self) -> Result<JsonAbi> {
        let mut project = self.project.clone();
        project.settings.update_output_selection(|selection| {
            *selection = OutputSelection::common_output_selection(["abi".to_string()])
        });

        let output = ProjectCompiler::new()
            .quiet(true)
            .files([self.target_path.clone()])
            .zksync_compile(&project, None)?;

        let artifact = output
            .find(&self.target_path, &self.target_name)
            .ok_or_eyre("failed to find target artifact when compiling for abi")?;

        artifact.abi.clone().ok_or_eyre("target artifact does not have an ABI")
    }

    /// Compiles target file requesting only metadata and returns it.
    pub fn get_target_metadata(&self) -> Result<serde_json::Value> {
        let mut project = self.project.clone();
        project.settings.update_output_selection(|selection| {
            *selection = OutputSelection::common_output_selection(["metadata".to_string()]);
        });

        let output = ProjectCompiler::new()
            .quiet(true)
            .files([self.target_path.clone()])
            .zksync_compile(&project, None)?;

        let artifact = output
            .find(&self.target_path, &self.target_name)
            .ok_or_eyre("failed to find target artifact when compiling for metadata")?;

        artifact.metadata.clone().ok_or_eyre("target artifact does not have an ABI")
    }

    /// Returns [Vec] containing imports of the target file.
    pub fn get_target_imports(&self) -> Result<Vec<PathBuf>> {
        let mut sources = self.project.paths.read_input_files()?;
        sources.insert(self.target_path.clone(), Source::read(&self.target_path)?);
        let graph = Graph::<SolData>::resolve_sources(&self.project.paths, sources)?;

        Ok(graph.imports(&self.target_path).into_iter().cloned().collect())
    }
}

#[derive(Debug)]
pub enum CompilerVerificationContext {
    Solc(VerificationContext),
    ZkSolc(ZkVerificationContext),
}

impl CompilerVerificationContext {
    pub fn config(&self) -> &Config {
        match self {
            Self::Solc(c) => &c.config,
            Self::ZkSolc(c) => &c.config,
        }
    }

    pub fn target_path(&self) -> &PathBuf {
        match self {
            Self::Solc(c) => &c.target_path,
            Self::ZkSolc(c) => &c.target_path,
        }
    }

    pub fn target_name(&self) -> &str {
        match self {
            Self::Solc(c) => &c.target_name,
            Self::ZkSolc(c) => &c.target_name,
        }
    }

    pub fn compiler_version(&self) -> &Version {
        match self {
            Self::Solc(c) => &c.compiler_version,
            // TODO: will refer to the solc version here. Analyze if we can remove
            // this ambiguity somehow (e.g: by having sepparate paths for solc/zksolc
            // and remove this method alltogether)
            Self::ZkSolc(c) => &c.compiler_version.solc,
        }
    }
    pub fn get_target_abi(&self) -> Result<JsonAbi> {
        match self {
            Self::Solc(c) => c.get_target_abi(),
            Self::ZkSolc(c) => c.get_target_abi(),
        }
    }
    pub fn get_target_imports(&self) -> Result<Vec<PathBuf>> {
        match self {
            Self::Solc(c) => c.get_target_imports(),
            Self::ZkSolc(c) => c.get_target_imports(),
        }
    }
    pub fn get_target_metadata(&self) -> Result<serde_json::Value> {
        match self {
            Self::Solc(c) => {
                let m = c.get_target_metadata()?;
                Ok(serde_json::to_value(m)?)
            }
            Self::ZkSolc(c) => c.get_target_metadata(),
        }
    }
}