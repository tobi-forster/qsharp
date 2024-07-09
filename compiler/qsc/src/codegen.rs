// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#[cfg(test)]
mod tests;

use qsc_codegen::qir::fir_to_qir;
use qsc_data_structures::{language_features::LanguageFeatures, target::TargetCapabilityFlags};
use qsc_frontend::{
    compile::{Dependencies, PackageStore, SourceMap},
    error::WithSource,
};
use qsc_partial_eval::ProgramEntry;
use qsc_passes::{PackageType, PassContext};

use crate::interpret::Error;

pub fn get_qir(
    sources: SourceMap,
    language_features: LanguageFeatures,
    capabilities: TargetCapabilityFlags,
    mut package_store: PackageStore,
    dependencies: &Dependencies,
) -> Result<String, Vec<Error>> {
    if capabilities == TargetCapabilityFlags::all() {
        return Err(vec![Error::UnsupportedRuntimeCapabilities]);
    }

    let (unit, errors) = crate::compile::compile(
        &package_store,
        dependencies,
        sources,
        PackageType::Exe,
        capabilities,
        language_features,
    );

    // Ensure it compiles before trying to add it to the store.
    if !errors.is_empty() {
        return Err(errors.iter().map(|e| Error::Compile(e.clone())).collect());
    }

    let package_id = package_store.insert(unit);
    let (fir_store, fir_package_id) = qsc_passes::lower_hir_to_fir(&package_store, package_id);
    let package = fir_store.get(fir_package_id);
    let entry = ProgramEntry {
        exec_graph: package.entry_exec_graph.clone(),
        expr: (
            fir_package_id,
            package
                .entry
                .expect("package must have an entry expression"),
        )
            .into(),
    };

    let compute_properties =
        PassContext::run_fir_passes_on_fir(&fir_store, fir_package_id, capabilities).map_err(
            |errors| {
                let source_package = package_store
                    .get(package_id)
                    .expect("package should be in store");
                errors
                    .iter()
                    .map(|e| Error::Pass(WithSource::from_map(&source_package.sources, e.clone())))
                    .collect::<Vec<_>>()
            },
        )?;

    fir_to_qir(&fir_store, capabilities, Some(compute_properties), &entry).map_err(|e| {
        let source_package_id = match e.span() {
            Some(span) => span.package,
            None => package_id,
        };
        let source_package = package_store
            .get(source_package_id)
            .expect("package should be in store");
        vec![Error::PartialEvaluation(WithSource::from_map(
            &source_package.sources,
            e,
        ))]
    })
}
