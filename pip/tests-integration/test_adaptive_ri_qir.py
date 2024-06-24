# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

import os

import pytest

from qsharp._native import (
    Interpreter,
    TargetProfile,
    QSharpError,
)

from typing import Optional, List


try:
    from qirrunner import run, OutputHandler

    QIR_RUNNER_AVAILABLE = True
except ImportError:
    QIR_RUNNER_AVAILABLE = False

SKIP_REASON = "QIR runner is not available"


def execute_qir(file_path: str) -> str:
    RNG_SEED = 42
    SHOTS = 1
    handler = OutputHandler()
    run(file_path, None, SHOTS, RNG_SEED, output_fn=handler.handle)
    return handler.get_output()


def get_interpreter(
    target_profile: TargetProfile = TargetProfile.Unrestricted,
    target_name: Optional[str] = None,
) -> Interpreter:
    if isinstance(target_name, str):
        target = target_name.split(".")[0].lower()
        if target == "ionq" or target == "rigetti":
            target_profile = TargetProfile.Base
        elif target == "quantinuum":
            target_profile = TargetProfile.Adaptive_RI
        else:
            raise QSharpError(
                f'target_name "{target_name}" not recognized. Please set target_profile directly.'
            )

    manifest_descriptor = None
    language_features = None
    from qsharp._fs import read_file, list_directory

    interpreter = Interpreter(
        target_profile,
        language_features,
        manifest_descriptor,
        read_file,
        list_directory,
    )
    return interpreter


def compile_qsharp(
    source: str,
    target_profile: TargetProfile = TargetProfile.Adaptive_RI,
    target_name: Optional[str] = None,
) -> str:
    interpreter = get_interpreter(target_profile, target_name)
    interpreter.interpret(source)
    qir = interpreter.qir("Test.Main()")
    return qir


def get_input_files() -> List[str]:
    resources_dir = get_resource_dir()
    input_files = [
        os.path.join(resources_dir, file_name)
        for file_name in os.listdir(resources_dir)
        if os.path.isfile(os.path.join(resources_dir, file_name))
    ]
    return input_files


def get_resource_dir() -> str:
    return os.path.join(os.path.dirname(__file__), "resources")


def get_output_dir() -> str:
    return os.path.join(get_resource_dir(), "output", "Adaptive_RI")


def get_ouput_file_basename(file_path: str) -> str:
    file_name, _ext = os.path.splitext(file_path)
    output_dir = get_output_dir()
    output_file = os.path.join(output_dir, os.path.basename(file_name))
    return output_file


def get_output_ll_file(file_path: str) -> str:
    output_file = get_ouput_file_basename(file_path)
    return output_file + ".ll"


def get_output_out_file(file_path: str) -> str:
    output_file = get_ouput_file_basename(file_path)
    return output_file + ".out"


# This function is used to generate the expected output files for the tests
# Rename the function to start with test_ to generate the expected output files
def generate_test_outputs() -> None:
    input_files = get_input_files()
    output_dir = get_output_dir()
    os.makedirs(output_dir, exist_ok=True)

    for file_path in input_files:
        ll_file_path = get_output_ll_file(file_path)
        out_file_path = get_output_out_file(file_path)
        with open(file_path, "rt", encoding="utf-8") as f:
            source = f.read()
            qir = compile_qsharp(source)
            with open(ll_file_path, "wt", encoding="utf-8") as f:
                f.write(qir)
            output = execute_qir(ll_file_path)
            with open(out_file_path, "wt", encoding="utf-8") as f:
                f.write(output)


def read_file(file_name: str) -> str:
    file_path = os.path.join(os.path.dirname(__file__), "resources", file_name)
    with open(file_path, "rt", encoding="utf-8") as file:
        source = file.read()
    return source


def save_qir_to_temp_file_and_execute(qir: str) -> str:

    import tempfile

    # create a temporary file to store the qir
    with tempfile.TemporaryDirectory() as tempdir:
        # Create a temporary file in the temporary directory
        with tempfile.NamedTemporaryFile(
            dir=tempdir, delete=True, suffix=".ll"
        ) as temp_file:
            # You can write to the file or read from it
            # encode the uf8 string to bytes
            temp_file.write(qir.encode())
            temp_file.flush()

            actual_output = execute_qir(temp_file.name)
            return actual_output


def assert_strings_equal_ignore_line_endings(lhs, rhs):
    normalized_lhs = lhs.replace("\r\n", "\n")
    normalized_rhs = rhs.replace("\r\n", "\n")
    assert normalized_lhs == normalized_rhs


@pytest.mark.parametrize("file_path", get_input_files())
@pytest.mark.skipif(not QIR_RUNNER_AVAILABLE, reason=SKIP_REASON)
def test_adaptive_ri_qir(file_path: str) -> None:
    source = read_file(file_path)
    ll_file_path = get_output_ll_file(file_path)
    expected_qir = read_file(ll_file_path)
    actual_qir = compile_qsharp(source)
    assert actual_qir == expected_qir


@pytest.mark.parametrize("file_path", get_input_files())
@pytest.mark.skipif(not QIR_RUNNER_AVAILABLE, reason=SKIP_REASON)
def test_adaptive_ri_output(file_path: str) -> None:
    source = read_file(file_path)
    qir = compile_qsharp(source)
    output_file_path = get_output_out_file(file_path)
    expected_output = read_file(output_file_path)
    actual_output = save_qir_to_temp_file_and_execute(qir)
    assert_strings_equal_ignore_line_endings(actual_output, expected_output)
