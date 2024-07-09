// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use miette::Diagnostic;
use qsc::{
    compile::ErrorKind,
    error::WithSource,
    line_column::{Encoding, Range},
    Span,
};

use crate::{
    compilation::Compilation,
    protocol::{CodeAction, CodeActionKind, TextEdit, WorkspaceEdit},
};

pub(crate) fn get_code_actions(
    compilation: &Compilation,
    source_name: &str,
    range: Range,
    position_encoding: Encoding,
) -> Vec<CodeAction> {
    // Compute quick_fixes and other code_actions, and then merge them together
    let span = compilation.source_range_to_package_span(source_name, range, position_encoding);
    quick_fixes(compilation, source_name, span, position_encoding)
}

fn quick_fixes(
    compilation: &Compilation,
    source_name: &str,
    span: Span,
    encoding: Encoding,
) -> Vec<CodeAction> {
    let mut code_actions = Vec::new();

    // get relevant diagnostics
    let diagnostics = compilation
        .compile_errors
        .iter()
        .filter(|error| is_error_relevant(error, span));

    // For all diagnostics that are lints, we extract the code action edits from them.
    for diagnostic in diagnostics {
        if let ErrorKind::Lint(lint) = diagnostic.error() {
            if !lint.code_action_edits.is_empty() {
                let source = compilation
                    .user_unit()
                    .sources
                    .find_by_name(source_name)
                    .expect("source should exist");
                let text_edits: Vec<TextEdit> = lint
                    .code_action_edits
                    .iter()
                    .map(|(new_text, span)| TextEdit {
                        new_text: new_text.clone(),
                        range: qsc::line_column::Range::from_span(encoding, &source.contents, span),
                    })
                    .collect();
                code_actions.push(CodeAction {
                    title: diagnostic.to_string(),
                    edit: Some(WorkspaceEdit {
                        changes: vec![(source_name.to_string(), text_edits)],
                    }),
                    kind: Some(CodeActionKind::QuickFix),
                    is_preferred: None,
                });
            }
        }
    }

    code_actions
}

/// Returns true if the error has a `Range` and it overlaps
/// with the code action's range.
fn is_error_relevant(error: &WithSource<ErrorKind>, span: Span) -> bool {
    let Some(error_span) = resolve_span(error) else {
        return false;
    };
    span.intersection(&error_span).is_some()
}

/// Extracts the uri and `Span` from an error.
fn resolve_span(e: &WithSource<ErrorKind>) -> Option<Span> {
    e.labels()
        .into_iter()
        .flatten()
        .map(|labeled_span| {
            let start = u32::try_from(labeled_span.offset()).expect("offset should fit in u32");
            let len = u32::try_from(labeled_span.len()).expect("length should fit in u32");
            qsc::Span {
                lo: start,
                hi: start + len,
            }
        })
        .next()
}
