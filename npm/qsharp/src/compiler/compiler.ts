// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

import { type Circuit as CircuitData } from "@microsoft/quantum-viz.js/lib/circuit.js";
import {
  IDocFile,
  IOperationInfo,
  IPackageGraphSources,
  IProgramConfig as wasmIProgramConfig,
  TargetProfile,
  type VSDiagnostic,
} from "../../lib/web/qsc_wasm.js";
import { log } from "../log.js";
import {
  IServiceProxy,
  ServiceProtocol,
  ServiceState,
} from "../workers/common.js";
import { eventStringToMsg } from "./common.js";
import {
  IQscEventTarget,
  QscEventData,
  QscEvents,
  makeEvent,
} from "./events.js";

// The wasm types generated for the node.js bundle are just the exported APIs,
// so use those as the set used by the shared compiler
type Wasm = typeof import("../../lib/web/qsc_wasm.js");

// These need to be async/promise results for when communicating across a WebWorker, however
// for running the compiler in the same thread the result will be synchronous (a resolved promise).
export interface ICompiler {
  checkCode(code: string): Promise<VSDiagnostic[]>;

  getAst(
    code: string,
    languageFeatures?: string[],
    profile?: TargetProfile,
  ): Promise<string>;

  getHir(
    code: string,
    languageFeatures?: string[],
    profile?: TargetProfile,
  ): Promise<string>;

  run(
    program: ProgramConfig,
    expr: string,
    shots: number,
    eventHandler: IQscEventTarget,
  ): Promise<void>;

  getQir(program: ProgramConfig): Promise<string>;

  getEstimates(program: ProgramConfig, params: string): Promise<string>;

  getCircuit(
    program: ProgramConfig,
    simulate: boolean,
    operation?: IOperationInfo,
  ): Promise<CircuitData>;

  getDocumentation(additionalProgram?: ProgramConfig): Promise<IDocFile[]>;

  checkExerciseSolution(
    userCode: string,
    exerciseSources: string[],
    eventHandler: IQscEventTarget,
  ): Promise<boolean>;
}

/**
 * Type definition for the configuration of a program.
 * If adding new properties, make them optional to maintain backward compatibility.
 */
export type ProgramConfig = (
  | {
      /** An array of source objects, each containing a name and contents. */
      sources: [string, string][];
      /** An array of language features to be opted in to in this compilation. */
      languageFeatures?: string[];
    }
  | {
      /** Sources from all resolved dependencies, along with their languageFeatures configuration */
      packageGraphSources: IPackageGraphSources;
    }
) & {
  /** Target compilation profile. */
  profile?: TargetProfile;
};

// WebWorker also support being explicitly terminated to tear down the worker thread
export type ICompilerWorker = ICompiler & IServiceProxy;
export type CompilerState = ServiceState;

export class Compiler implements ICompiler {
  private wasm: Wasm;

  constructor(wasm: Wasm) {
    log.info("Constructing a Compiler instance");
    this.wasm = wasm;
    globalThis.qscGitHash = this.wasm.git_hash();
  }

  // Note: This function does not support project mode.
  // see https://github.com/microsoft/qsharp/pull/849#discussion_r1409821143
  async checkCode(code: string): Promise<VSDiagnostic[]> {
    let diags: VSDiagnostic[] = [];
    const languageService = new this.wasm.LanguageService();
    const work = languageService.start_background_work(
      (uri: string, version: number | undefined, errors: VSDiagnostic[]) => {
        diags = errors;
      },
      {
        readFile: async () => null,
        listDirectory: async () => [],
        resolvePath: async () => null,
        fetchGithub: async () => "",
        findManifestDirectory: async () => null,
      },
    );
    languageService.update_document("code", 1, code);
    // Yield to let the language service background worker handle the update
    await Promise.resolve();
    languageService.stop_background_work();
    await work;
    languageService.free();
    return diags;
  }

  async getAst(
    code: string,
    languageFeatures?: string[],
    profile?: TargetProfile,
  ): Promise<string> {
    return this.wasm.get_ast(
      code,
      languageFeatures ?? [],
      profile ?? "adaptive_ri",
    );
  }

  async getHir(
    code: string,
    languageFeatures: string[],
    profile: TargetProfile,
  ): Promise<string> {
    return this.wasm.get_hir(
      code,
      languageFeatures ?? [],
      profile ?? "adaptive_ri",
    );
  }

  async run(
    program: ProgramConfig,
    expr: string,
    shots: number,
    eventHandler: IQscEventTarget,
  ): Promise<void> {
    // All results are communicated as events, but if there is a compiler error (e.g. an invalid
    // entry expression or similar), it may throw on run. The caller should expect this promise
    // may reject without all shots running or events firing.
    this.wasm.run(
      toWasmProgramConfig(program, "unrestricted"),
      expr,
      (msg: string) => onCompilerEvent(msg, eventHandler!),
      shots!,
    );
  }

  async getQir(program: ProgramConfig): Promise<string> {
    return this.wasm.get_qir(toWasmProgramConfig(program, "base"));
  }

  async getEstimates(program: ProgramConfig, params: string): Promise<string> {
    return this.wasm.get_estimates(
      toWasmProgramConfig(program, "unrestricted"),
      params,
    );
  }

  async getCircuit(
    program: ProgramConfig,
    simulate: boolean,
    operation?: IOperationInfo,
  ): Promise<CircuitData> {
    return this.wasm.get_circuit(
      toWasmProgramConfig(program, "unrestricted"),
      simulate,
      operation,
    );
  }

  // Returns all autogenerated documentation files for the standard library
  // and loaded project (if requested). This include file names and metadata,
  // including specially formatted table of content file.
  async getDocumentation(
    additionalProgram?: ProgramConfig,
  ): Promise<IDocFile[]> {
    return this.wasm.generate_docs(
      additionalProgram &&
        toWasmProgramConfig(additionalProgram, "unrestricted"),
    );
  }

  async checkExerciseSolution(
    userCode: string,
    exerciseSources: string[],
    eventHandler: IQscEventTarget,
  ): Promise<boolean> {
    const success = this.wasm.check_exercise_solution(
      userCode,
      exerciseSources,
      (msg: string) => onCompilerEvent(msg, eventHandler),
    );

    return success;
  }
}

/**
 * Fills in the defaults, to convert from the backwards-compatible ProgramConfig,
 * to the IProgramConfig type that the wasm layer expects
 */
export function toWasmProgramConfig(
  program: ProgramConfig,
  defaultProfile: TargetProfile,
): Required<wasmIProgramConfig> {
  let packageGraphSources: IPackageGraphSources;

  if ("sources" in program) {
    // The simpler type is used, where there are no dependencies and only a list
    // of sources is passed in.
    packageGraphSources = {
      root: {
        sources: program.sources,
        languageFeatures: program.languageFeatures || [],
        dependencies: {},
      },
      packages: {},
    };
  } else {
    // A full package graph is passed in.
    packageGraphSources = program.packageGraphSources;
  }

  return { packageGraphSources, profile: program.profile || defaultProfile };
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

/** The protocol definition to allow running the compiler in a worker. */
export const compilerProtocol: ServiceProtocol<ICompiler, QscEventData> = {
  class: Compiler,
  methods: {
    checkCode: "request",
    getAst: "request",
    getHir: "request",
    getQir: "request",
    getEstimates: "request",
    getCircuit: "request",
    getDocumentation: "request",
    run: "requestWithProgress",
    checkExerciseSolution: "requestWithProgress",
  },
  eventNames: ["DumpMachine", "Message", "Result"],
};
