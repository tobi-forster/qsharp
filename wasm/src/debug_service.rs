// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use crate::line_column::{Location, Range};
use crate::project_system::{into_qsc_args, ProgramConfig};
use crate::{serializable_type, CallbackReceiver};
use qsc::fir::StmtId;
use qsc::fmt_complex;
use qsc::interpret::{Debugger, Error, StepAction, StepResult};
use qsc::line_column::Encoding;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Default)]
pub struct DebugService {
    debugger: Option<Debugger>,
}

#[wasm_bindgen]
impl DebugService {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::needless_pass_by_value)] // needed for wasm_bindgen
    pub fn load_program(&mut self, program: ProgramConfig, entry: Option<String>) -> String {
        match init_debugger(program, entry) {
            Ok(debugger) => {
                self.debugger = Some(debugger);
                String::new()
            }
            Err(e) => render_errors(e),
        }
    }

    pub fn capture_quantum_state(&mut self) -> IQuantumStateList {
        let state = self.debugger_mut().capture_quantum_state();
        let entries = state
            .0
            .iter()
            .map(|(id, value)| QuantumState {
                name: qsc::format_state_id(id, state.1),
                value: fmt_complex(value),
            })
            .collect::<Vec<_>>();

        QuantumStateList { entries }.into()
    }

    pub fn get_circuit(&self) -> Result<JsValue, String> {
        let circuit = self.debugger().circuit();
        serde_wasm_bindgen::to_value(&circuit).map_err(|e| e.to_string())
    }

    pub fn get_stack_frames(&self) -> IStackFrameList {
        let frames = self.debugger().get_stack_frames();

        StackFrameList {
            frames: frames
                .into_iter()
                .map(|s| StackFrame {
                    name: format!("{} {}", s.name, s.functor),
                    location: s.location.into(),
                })
                .collect(),
        }
        .into()
    }

    pub fn eval_next(
        &mut self,
        event_cb: &js_sys::Function,
        ids: &[u32],
    ) -> Result<IStructStepResult, JsValue> {
        self.eval(event_cb, ids, StepAction::Next)
    }

    pub fn eval_continue(
        &mut self,
        event_cb: &js_sys::Function,
        ids: &[u32],
    ) -> Result<IStructStepResult, JsValue> {
        self.eval(event_cb, ids, StepAction::Continue)
    }

    pub fn eval_step_in(
        &mut self,
        event_cb: &js_sys::Function,
        ids: &[u32],
    ) -> Result<IStructStepResult, JsValue> {
        self.eval(event_cb, ids, StepAction::In)
    }

    pub fn eval_step_out(
        &mut self,
        event_cb: &js_sys::Function,
        ids: &[u32],
    ) -> Result<IStructStepResult, JsValue> {
        self.eval(event_cb, ids, StepAction::Out)
    }

    fn eval(
        &mut self,
        event_cb: &js_sys::Function,
        ids: &[u32],
        step: StepAction,
    ) -> Result<IStructStepResult, JsValue> {
        if !event_cb.is_function() {
            return Err(JsError::new("Events callback function must be provided").into());
        }
        let bps: Vec<_> = ids.iter().map(|f| StmtId::from(*f)).collect();

        let event_cb = |msg: &str| {
            // See example at https://rustwasm.github.io/wasm-bindgen/reference/receiving-js-closures-in-rust.html
            let _ = event_cb.call1(&JsValue::null(), &JsValue::from(msg));
        };
        match self.run_internal(event_cb, &bps, step) {
            Ok(value) => Ok(StructStepResult::from(value).into()),
            Err(e) => Err(JsError::from(&e[0]).into()),
        }
    }

    fn run_internal<F>(
        &mut self,
        event_cb: F,
        bps: &[StmtId],
        step: StepAction,
    ) -> Result<StepResult, Vec<Error>>
    where
        F: Fn(&str),
    {
        let mut out = CallbackReceiver { event_cb };
        let result = self.debugger_mut().eval_step(&mut out, bps, step);
        let mut success = true;

        let msg: Option<serde_json::Value> = match &result {
            Ok(value) => match value {
                qsc::interpret::StepResult::Return(value) => {
                    Some(serde_json::Value::String(value.to_string()))
                }
                _ => None,
            },
            Err(errors) => {
                // TODO: handle multiple errors
                // https://github.com/microsoft/qsharp/issues/149
                success = false;
                errors[0]
                    .stack_trace()
                    .clone()
                    .map(serde_json::Value::String)
            }
        };
        if let Some(value) = msg {
            let msg_string =
                json!({"type": "Result", "success": success, "result": value}).to_string();
            (out.event_cb)(&msg_string);
        }

        match result {
            Ok(value) => Ok(value),
            Err(errors) => Err(errors.clone()),
        }
    }

    pub fn get_breakpoints(&self, path: &str) -> IBreakpointSpanList {
        let bps = self.debugger().get_breakpoints(path);

        BreakpointSpanList {
            spans: bps
                .iter()
                .map(|s| BreakpointSpan {
                    id: s.id,
                    range: s.range.into(),
                })
                .collect(),
        }
        .into()
    }

    pub fn get_locals(&self) -> IVariableList {
        let locals = self.debugger().get_locals();
        let variables: Vec<_> = locals
            .into_iter()
            .map(|local| Variable {
                name: (*local.name).to_string(),
                value: local.value.to_string(),
                var_type: local.type_name,
            })
            .collect();
        VariableList { variables }.into()
    }

    fn debugger(&self) -> &Debugger {
        self.debugger
            .as_ref()
            .expect("debugger should be initialized")
    }

    fn debugger_mut(&mut self) -> &mut Debugger {
        self.debugger
            .as_mut()
            .expect("debugger should be initialized")
    }
}

pub fn init_debugger(
    program: ProgramConfig,
    entry: Option<String>,
) -> Result<Debugger, Vec<Error>> {
    let (source_map, capabilities, language_features, package_store, user_code_dependencies) =
        into_qsc_args(program, entry)
            .map_err(|e| e.into_iter().map(Into::into).collect::<Vec<_>>())?;

    Debugger::new(
        source_map,
        capabilities,
        Encoding::Utf16,
        language_features,
        package_store,
        &user_code_dependencies[..],
    )
}

fn render_errors(errors: Vec<Error>) -> String {
    let mut msg = String::new();
    for error in errors {
        let error_string = render_error(error);
        msg.push_str(&error_string);
    }
    msg
}

fn render_error(error: Error) -> String {
    format!("{:?}\n", miette::Report::new(error))
}

impl From<StepResult> for StructStepResult {
    fn from(value: StepResult) -> Self {
        match value {
            StepResult::BreakpointHit(value) => StructStepResult {
                id: StepResultId::BreakpointHit.into(),
                value: Into::<usize>::into(value),
            },
            StepResult::Next => StructStepResult {
                id: StepResultId::Next.into(),
                value: 0,
            },
            StepResult::StepIn => StructStepResult {
                id: StepResultId::StepIn.into(),
                value: 0,
            },
            StepResult::StepOut => StructStepResult {
                id: StepResultId::StepOut.into(),
                value: 0,
            },
            StepResult::Return(_) => StructStepResult {
                id: StepResultId::Return.into(),
                value: 0,
            },
        }
    }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum StepResultId {
    BreakpointHit = 0,
    Next = 1,
    StepIn = 2,
    StepOut = 3,
    Return = 4,
}

impl From<StepResultId> for usize {
    fn from(val: StepResultId) -> Self {
        val as usize
    }
}

serializable_type! {
    StructStepResult,
    {
        pub id: usize,
        pub value: usize,
    },
    r#"export interface IStructStepResult {
        id: number;
        value: number;
    }"#,
    IStructStepResult
}

serializable_type! {
    BreakpointSpanList,
    {
        pub spans: Vec<BreakpointSpan>,
    },
    r#"export interface IBreakpointSpanList {
        spans: Array<IBreakpointSpan>
    }
    "#,
    IBreakpointSpanList
}

serializable_type! {
    BreakpointSpan,
    {
        pub id: u32,
        pub range: Range
    },
    r#"export interface IBreakpointSpan {
        id: number;
        range: IRange;
    }"#
}

serializable_type! {
    StackFrameList,
    {
        pub frames: Vec<StackFrame>,
    },
    r#"export interface IStackFrameList {
        frames: Array<IStackFrame>
    }
    "#,
    IStackFrameList
}

serializable_type! {
    StackFrame,
    {
        pub name: String,
        pub location: Location
    },
    r#"export interface IStackFrame {
        name: string;
        location: ILocation
    }"#
}

serializable_type! {
    VariableList,
    {
        pub variables: Vec<Variable>,
    },
    r#"export interface IVariableList {
        variables: Array<IVariable>
    }"#,
    IVariableList
}

serializable_type! {
    Variable,
    {
        pub name: String,
        pub value: String,
        pub var_type: String,
    },
    r#"export interface IVariable {
        name: string;
        value: string;
        var_type: "Array"
            | "BigInt"
            | "Bool"
            | "Closure"
            | "Double"
            | "Global"
            | "Int"
            | "Pauli"
            | "Qubit"
            | "Range"
            | "Result"
            | "String"
            | "Tuple";
    }"#
}

serializable_type! {
    QuantumStateList,
    {
        pub entries: Vec<QuantumState>,
    },
    r#"export interface IQuantumStateList {
        entries: Array<IQuantumState>
    }"#,
    IQuantumStateList
}

serializable_type! {
    QuantumState,
    {
        pub name: String,
        pub value: String,
    },
    r#"export interface IQuantumState {
        name: string;
        value: string;
    }"#
}
