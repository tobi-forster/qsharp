// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

/// <reference types="user-agent-data-types" />

import * as vscode from "vscode";
import TelemetryReporter from "@vscode/extension-telemetry";
import { log } from "qsharp-lang";

export enum EventType {
  InitializePlugin = "Qsharp.InitializePlugin",
  LoadLanguageService = "Qsharp.LoadLanguageService",
  ReturnCompletionList = "Qsharp.ReturnCompletionList",
  GenerateQirStart = "Qsharp.GenerateQirStart",
  GenerateQirEnd = "Qsharp.GenerateQirEnd",
  RenderQuantumStateStart = "Qsharp.RenderQuantumStateStart",
  RenderQuantumStateEnd = "Qsharp.RenderQuantumStateEnd",
  SubmitToAzureStart = "Qsharp.SubmitToAzureStart",
  SubmitToAzureEnd = "Qsharp.SubmitToAzureEnd",
  AuthSessionStart = "Qsharp.AuthSessionStart",
  AuthSessionEnd = "Qsharp.AuthSessionEnd",
  QueryWorkspacesStart = "Qsharp.QueryWorkspacesStart",
  QueryWorkspacesEnd = "Qsharp.QueryWorkspacesEnd",
  AzureRequestFailed = "Qsharp.AzureRequestFailed",
  StorageRequestFailed = "Qsharp.StorageRequestFailed",
  GetJobFilesStart = "Qsharp.GetJobFilesStart",
  GetJobFilesEnd = "Qsharp.GetJobFilesEnd",
  QueryWorkspaceStart = "Qsharp.QueryWorkspaceStart",
  QueryWorkspaceEnd = "Qsharp.QueryWorkspaceEnd",
  CheckCorsStart = "Qsharp.CheckCorsStart",
  CheckCorsEnd = "Qsharp.CheckCorsEnd",
  InitializeRuntimeStart = "Qsharp.InitializeRuntimeStart",
  InitializeRuntimeEnd = "Qsharp.InitializeRuntimeEnd",
  DebugSessionEvent = "Qsharp.DebugSessionEvent",
  Launch = "Qsharp.Launch",
  OpenedDocument = "Qsharp.OpenedDocument",
  TriggerResourceEstimation = "Qsharp.TriggerResourceEstimation",
  ResourceEstimationStart = "Qsharp.ResourceEstimationStart",
  ResourceEstimationEnd = "Qsharp.ResourceEstimationEnd",
  TriggerHistogram = "Qsharp.TriggerHistogram",
  HistogramStart = "Qsharp.HistogramStart",
  HistogramEnd = "Qsharp.HistogramEnd",
  FormatStart = "Qsharp.FormatStart",
  FormatEnd = "Qsharp.FormatEnd",
  CreateProject = "Qsharp.CreateProject",
  TriggerCircuit = "Qsharp.TriggerCircuit",
  CircuitStart = "Qsharp.CircuitStart",
  CircuitEnd = "Qsharp.CircuitEnd",
}

type Empty = { [K in any]: never };

type EventTypes = {
  [EventType.InitializePlugin]: {
    properties: Empty;
    measurements: Empty;
  };
  [EventType.LoadLanguageService]: {
    properties: Empty;
    measurements: {
      timeToStartMs: number;
    };
  };
  [EventType.ReturnCompletionList]: {
    properties: Empty;
    measurements: { timeToCompletionMs: number; completionListLength: number };
  };
  [EventType.GenerateQirStart]: {
    properties: { associationId: string; targetProfile: string };
    measurements: Empty;
  };
  [EventType.GenerateQirEnd]: {
    properties: { associationId: string };
    measurements: { qirLength: number; timeToCompleteMs: number };
  };
  [EventType.RenderQuantumStateStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.RenderQuantumStateEnd]: {
    properties: { associationId: string };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.SubmitToAzureStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.SubmitToAzureEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.AuthSessionStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.AuthSessionEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.QueryWorkspacesStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.QueryWorkspacesEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.AzureRequestFailed]: {
    properties: { associationId: string; reason?: string };
    measurements: Empty;
  };
  [EventType.StorageRequestFailed]: {
    properties: { associationId: string; reason?: string };
    measurements: Empty;
  };
  [EventType.GetJobFilesStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.GetJobFilesEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.QueryWorkspaceStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.QueryWorkspaceEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.CheckCorsStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.CheckCorsEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.InitializeRuntimeStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.InitializeRuntimeEnd]: {
    properties: {
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.DebugSessionEvent]: {
    properties: {
      associationId: string;
      event: DebugEvent;
    };
    measurements: Empty;
  };
  [EventType.Launch]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.OpenedDocument]: {
    properties: { documentType: QsharpDocumentType };
    measurements: { linesOfCode: number };
  };
  [EventType.TriggerResourceEstimation]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.ResourceEstimationStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.ResourceEstimationEnd]: {
    properties: { associationId: string };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.TriggerHistogram]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.HistogramStart]: {
    properties: { associationId: string };
    measurements: Empty;
  };
  [EventType.HistogramEnd]: {
    properties: { associationId: string };
    measurements: { timeToCompleteMs: number };
  };
  [EventType.FormatStart]: {
    properties: { associationId: string; event: FormatEvent };
    measurements: Empty;
  };
  [EventType.FormatEnd]: {
    properties: { associationId: string };
    measurements: { timeToCompleteMs: number; numberOfEdits: number };
  };
  [EventType.CreateProject]: {
    properties: Empty;
    measurements: Empty;
  };
  [EventType.TriggerCircuit]: {
    properties: {
      associationId: string;
    };
    measurements: Empty;
  };
  [EventType.CircuitStart]: {
    properties: {
      associationId: string;
      isOperation: string;
      targetProfile: string;
    };
    measurements: Empty;
  };
  [EventType.CircuitEnd]: {
    properties: {
      simulated: string;
      associationId: string;
      reason?: string;
      flowStatus: UserFlowStatus;
    };
    measurements: { timeToCompleteMs: number };
  };
};

export enum QsharpDocumentType {
  JupyterCell = "JupyterCell",
  Qsharp = "Qsharp",
  Other = "Other",
}

export enum UserFlowStatus {
  // "Aborted" means the flow was intentionally canceled or left, either by us or the user
  Aborted = "Aborted",
  Succeeded = "Succeeded",
  // "CompletedWithFailure" means something that we can action -- service request failure, exceptions, etc.
  Failed = "Failed",
}

export enum DebugEvent {
  StepIn = "StepIn",
  Continue = "Continue",
}

export enum FormatEvent {
  OnDocument = "OnDocument",
  OnRange = "OnRange",
  OnType = "OnType",
}

let reporter: TelemetryReporter | undefined;
let userAgentString: string | undefined;

export function initTelemetry(context: vscode.ExtensionContext) {
  const packageJson = context.extension?.packageJSON;
  if (!packageJson) {
    return;
  }
  reporter = new TelemetryReporter(packageJson.aiKey);
  const version = context.extension?.packageJSON?.version;
  const browserAndRelease = getBrowserRelease();
  userAgentString = `VSCode/${version} ${browserAndRelease}`;

  sendTelemetryEvent(EventType.InitializePlugin, {}, {});
}

export function sendTelemetryEvent<E extends keyof EventTypes>(
  event: E,
  properties: EventTypes[E]["properties"] = {},
  measurements: EventTypes[E]["measurements"] = {},
) {
  if (reporter === undefined) {
    log.trace(`No telemetry reporter. Omitting telemetry event ${event}`);
    return;
  }

  // If you get a type error here, it's likely because you defined a
  // non-string property or non-number measurement in `EventTypes`.
  // For booleans, use `.toString()` to convert to string and store in `properties`.
  reporter.sendTelemetryEvent(event, properties, measurements);
  log.debug(
    `Sent telemetry: ${event} ${JSON.stringify(properties)} ${JSON.stringify(
      measurements,
    )}`,
  );
}

function getBrowserRelease(): string {
  if (navigator.userAgentData?.brands) {
    const browser =
      navigator.userAgentData.brands[navigator.userAgentData.brands.length - 1];
    return `${browser.brand}/${browser.version}`;
  } else {
    return navigator.userAgent;
  }
}

export function getUserAgent(): string {
  return userAgentString || navigator.userAgent;
}
