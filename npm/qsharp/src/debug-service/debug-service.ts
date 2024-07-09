// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

import { type Circuit as CircuitData } from "@microsoft/quantum-viz.js/lib/circuit.js";
import type {
  DebugService,
  IBreakpointSpan,
  IQuantumState,
  IStackFrame,
  IStructStepResult,
  IVariable,
} from "../../lib/web/qsc_wasm.js";
import { ProgramConfig } from "../browser.js";
import { eventStringToMsg } from "../compiler/common.js";
import {
  IQscEventTarget,
  QscEventData,
  QscEvents,
  makeEvent,
} from "../compiler/events.js";
import { log } from "../log.js";
import { IServiceProxy, ServiceProtocol } from "../workers/common.js";
import { toWasmProgramConfig } from "../compiler/compiler.js";

type QscWasm = typeof import("../../lib/web/qsc_wasm.js");

// These need to be async/promise results for when communicating across a WebWorker, however
// for running the debugger in the same thread the result will be synchronous (a resolved promise).
export interface IDebugService {
  loadProgram(
    program: ProgramConfig,
    entry: string | undefined,
  ): Promise<string>;
  getBreakpoints(path: string): Promise<IBreakpointSpan[]>;
  getLocalVariables(): Promise<Array<IVariable>>;
  captureQuantumState(): Promise<Array<IQuantumState>>;
  getCircuit(): Promise<CircuitData>;
  getStackFrames(): Promise<IStackFrame[]>;
  evalContinue(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult>;
  evalNext(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult>;
  evalStepIn(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult>;
  evalStepOut(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult>;
  dispose(): Promise<void>;
}

export type IDebugServiceWorker = IDebugService & IServiceProxy;

export class QSharpDebugService implements IDebugService {
  private wasm: QscWasm;
  private debugService: DebugService;

  constructor(wasm: QscWasm) {
    log.info("Constructing a QSharpDebugService instance");
    this.wasm = wasm;
    this.debugService = new wasm.DebugService();
  }

  async loadProgram(
    program: ProgramConfig,
    entry: string | undefined,
  ): Promise<string> {
    return this.debugService.load_program(
      toWasmProgramConfig(program, "unrestricted"),
      entry,
    );
  }

  async getBreakpoints(path: string): Promise<IBreakpointSpan[]> {
    return this.debugService.get_breakpoints(path).spans;
  }

  async getLocalVariables(): Promise<Array<IVariable>> {
    const variable_list = this.debugService.get_locals();
    return variable_list.variables;
  }

  async captureQuantumState(): Promise<Array<IQuantumState>> {
    const state = this.debugService.capture_quantum_state();
    return state.entries;
  }

  async getCircuit(): Promise<CircuitData> {
    return this.debugService.get_circuit();
  }

  async getStackFrames(): Promise<IStackFrame[]> {
    return this.debugService.get_stack_frames().frames;
  }

  async evalContinue(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult> {
    const event_cb = (msg: string) => onCompilerEvent(msg, eventHandler);
    const ids = new Uint32Array(bps);
    return this.debugService.eval_continue(event_cb, ids);
  }

  async evalNext(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult> {
    const event_cb = (msg: string) => onCompilerEvent(msg, eventHandler);
    const ids = new Uint32Array(bps);
    return this.debugService.eval_next(event_cb, ids);
  }

  async evalStepIn(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult> {
    const event_cb = (msg: string) => onCompilerEvent(msg, eventHandler);
    const ids = new Uint32Array(bps);
    return this.debugService.eval_step_in(event_cb, ids);
  }

  async evalStepOut(
    bps: number[],
    eventHandler: IQscEventTarget,
  ): Promise<IStructStepResult> {
    const event_cb = (msg: string) => onCompilerEvent(msg, eventHandler);
    const ids = new Uint32Array(bps);
    return this.debugService.eval_step_out(event_cb, ids);
  }

  async dispose() {
    this.debugService.free();
  }
}

export function onCompilerEvent(msg: string, eventTarget: IQscEventTarget) {
  const qscMsg = eventStringToMsg(msg);
  if (!qscMsg) {
    log.error("Unknown event message: %s", msg);
    return;
  }

  let qscEvent: QscEvents;

  const msgType = qscMsg.type;
  switch (msgType) {
    case "Message":
      qscEvent = makeEvent("Message", qscMsg.message);
      break;
    case "DumpMachine":
      qscEvent = makeEvent("DumpMachine", {
        state: qscMsg.state,
        stateLatex: qscMsg.stateLatex,
      });
      break;
    case "Result":
      qscEvent = makeEvent("Result", qscMsg.result);
      break;
    default:
      log.never(msgType);
      throw "Unexpected message type";
  }
  log.debug("worker dispatching event " + JSON.stringify(qscEvent));
  eventTarget.dispatchEvent(qscEvent);
}

/** The protocol definition to allow running the debugger in a worker. */
export const debugServiceProtocol: ServiceProtocol<
  IDebugService,
  QscEventData
> = {
  class: QSharpDebugService,
  methods: {
    loadProgram: "request",
    getBreakpoints: "request",
    getLocalVariables: "request",
    captureQuantumState: "request",
    getCircuit: "request",
    getStackFrames: "request",
    evalContinue: "requestWithProgress",
    evalNext: "requestWithProgress",
    evalStepIn: "requestWithProgress",
    evalStepOut: "requestWithProgress",
    dispose: "request",
  },
  eventNames: ["DumpMachine", "Message", "Result"],
};
