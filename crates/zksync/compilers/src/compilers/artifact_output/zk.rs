//! ZK Sync artifact output
use crate::artifacts::contract::Contract;
use alloy_json_abi::JsonAbi;
use foundry_compilers::{
    artifacts::{DevDoc, SourceFile, StorageLayout, UserDoc},
    sources::VersionedSourceFile,
    ArtifactOutput,
};
use foundry_compilers_artifacts_solc::{
    CompactBytecode, CompactContract, CompactContractBytecode, CompactContractBytecodeCow,
    CompactDeployedBytecode,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap, path::Path};

mod bytecode;
pub use bytecode::ZkArtifactBytecode;

/// Artifact representing a compiled contract
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZkContractArtifact {
    /// contract abi
    pub abi: Option<JsonAbi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// contract bytecodee
    pub bytecode: Option<ZkArtifactBytecode>,
    /// contract assembly
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// contract metadata
    pub metadata: Option<serde_json::Value>,
    /// contract storage layout
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_layout: Option<StorageLayout>,
    /// contract userdoc
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub userdoc: Option<UserDoc>,
    /// contract devdoc
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devdoc: Option<DevDoc>,
    /// contract optimized IR code
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_optimized: Option<String>,
    /// contract hash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// contract factory dependencies
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factory_dependencies: Option<BTreeMap<String, String>>,
    /// The identifier of the source file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
}

impl ZkContractArtifact {
    /// Get contract missing libraries
    pub fn missing_libraries(&self) -> Option<&Vec<String>> {
        self.bytecode.as_ref().map(|bc| &bc.missing_libraries)
    }
}

// CompactContract variants
// TODO: for zkEvm, the distinction between bytecode and deployed_bytecode makes little sense,
// and there some fields that the output doesn't provide (e.g: source_map)
// However, we implement these because we get the Artifact trait and can reuse lots of
// the crate's helpers without needing to duplicate everything. Maybe there's a way
// we can get all these without having to add the same bytecode twice on each struct.
// Ideally the Artifacts trait would not be coupled to a specific Contract type
impl<'a> From<&'a ZkContractArtifact> for CompactContractBytecodeCow<'a> {
    fn from(artifact: &'a ZkContractArtifact) -> Self {
        // TODO: artifact.abi might have None, we need to get this field from solc_metadata
        CompactContractBytecodeCow {
            abi: artifact.abi.as_ref().map(Cow::Borrowed),
            bytecode: artifact.bytecode.clone().map(|b| Cow::Owned(CompactBytecode::from(b))),
            deployed_bytecode: artifact
                .bytecode
                .clone()
                .map(|b| Cow::Owned(CompactDeployedBytecode::from(b))),
        }
    }
}

impl From<ZkContractArtifact> for CompactContractBytecode {
    fn from(c: ZkContractArtifact) -> Self {
        Self {
            abi: c.abi.map(Into::into),
            deployed_bytecode: c.bytecode.clone().map(|b| b.into()),
            bytecode: c.bytecode.clone().map(|b| b.into()),
        }
    }
}

impl From<ZkContractArtifact> for CompactContract {
    fn from(c: ZkContractArtifact) -> Self {
        // TODO: c.abi might have None, we need to get this field from solc_metadata
        Self {
            bin: c.bytecode.clone().map(|b| b.object()),
            bin_runtime: c.bytecode.clone().map(|b| b.object()),
            abi: c.abi,
        }
    }
}

/// ZK Sync ArtifactOutput
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct ZkArtifactOutput();

impl ArtifactOutput for ZkArtifactOutput {
    type Artifact = ZkContractArtifact;
    type CompilerContract = Contract;

    fn contract_to_artifact(
        &self,
        _file: &Path,
        _name: &str,
        contract: Self::CompilerContract,
        source_file: Option<&SourceFile>,
    ) -> Self::Artifact {
        let is_unlinked = contract.is_unlinked();
        let Contract {
            abi,
            metadata,
            userdoc,
            devdoc,
            storage_layout,
            eravm,
            evm,
            ir_optimized,
            hash,
            factory_dependencies,
            missing_libraries,
        } = contract;

        let (bytecode, assembly) = eravm
            .map(|eravm| (eravm.bytecode(is_unlinked), eravm.assembly))
            .or_else(|| evm.map(|evm| (evm.bytecode.map(|bc| bc.object), evm.assembly)))
            .unwrap_or_else(|| (None, None));
        let bytecode = bytecode
            .map(|object| ZkArtifactBytecode::with_object(object, is_unlinked, missing_libraries));

        ZkContractArtifact {
            abi,
            hash,
            factory_dependencies,
            storage_layout: Some(storage_layout),
            bytecode,
            assembly,
            metadata,
            userdoc: Some(userdoc),
            devdoc: Some(devdoc),
            ir_optimized,
            id: source_file.as_ref().map(|s| s.id),
        }
    }

    fn standalone_source_file_to_artifact(
        &self,
        _path: &Path,
        _file: &VersionedSourceFile,
    ) -> Option<Self::Artifact> {
        None
    }
}