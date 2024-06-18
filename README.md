# Azure Quantum Development Kit

Welcome to the Azure Quantum Development Kit!

This repository contains tooling for the Q# language, specifically:

- **[compiler](./compiler/qsc/)**: core compiler logic and command-line tooling
- **[fuzz](./fuzz/)**: fuzz testing infrastructure
- **[jupyterlab](./jupyterlab/)**: JupyterLab extension
- **[language_service](./language_service/)**: Q# language service and editor features
- **[library](./library/)**: Q# standard library
- **[npm](./npm/)**: Q# npm package
- **[pip](./pip/)**: Q# Python pip package
- **[playground](./playground/)**: simple website for interacting with Q#
- **[resource_estimator](./resource_estimator)**: Implementation for the Azure Quantum Resource Estimator
- **[vscode](./vscode/)**: Visual Studio Code extension
- **[wasm](./wasm/)**: The bindings and logic for the WebAssembly module
- **[widgets](./widgets)**: The Q# Jupyter widgets Python package

There are also the tutorials and samples in the `./katas` and `./samples` directories, respectively.

Code from this repository powers the Q# development experience on <https://quantum.microsoft.com>.

## Building

To build this repository there are dependencies that need to be installed. These are:

- Python (<https://python.org>)
- Rust (<https://www.rust-lang.org/tools/install>)
  - On all platforms, the `wasm32-unknown-unknown` must be installed to build the WASM based components
    ```shell
    rustup target add wasm32-unknown-unknown
    ```
  - On MacOS, ensure that both `aarch64` and `x86_64` targets are installed or you will encounter linking errors.
    ```shell
    rustup target add x86_64-apple-darwin
    rustup target add aarch64-apple-darwin
    ```
- Node.js (<https://nodejs.org/>)
- wasm-pack (<https://rustwasm.github.io/wasm-pack/installer/>)
- A [C compiler](https://docs.rs/cc/latest/cc/#compile-time-requirements)

The build script will check these dependencies and their versions and fail if not met. (Or run
`python ./prereqs.py` directly to check if the minimum required versions are installed).

To build, in the root directory run `python ./build.py`. By default this will run a release
build of each project, including running tests and checks such as linting. Run with the
`--help` option for detailed usage.

### Playground

The `playground` is a small website that loads the Q# editor, compiler, samples, katas, and documentation for the standard library. It's a way to manually validate any changes you make to these components.

To see instructions for building the playground, refer to [Building the Playground Locally](./playground/README.md#building-the-playground-locally).

### Python

When building the Python packages (`pip` and `jupyterlab`), if the build script does not detect
a current Python virtual environment, it will automatically create one under `pip/.venv` or
`jupyterlab/.venv`. When developing locally, you can use these virtual environments to run the
tests by running `source .venv/bin/activate` (Linux/MacOS) or `.venv/Scripts/activate.bat` (Windows).

## Code editing

The easiest way to develop in this repo is to use VS Code. When you open the project root, by
default VS Code will recommend you install the extensions listed in `.vscode/extensions.json`.
These extensions provide language services for editing, as well as linters and formatters to
ensure the code meets the requirements (which are checked by the `build.py` script and CI).

Some settings are recommended (but not enforced) to make development easier. These are in the
`.vscode/*.shared.json` files. If the [Workspace Config+](https://marketplace.visualstudio.com/items?itemName=swellaby.workspace-config-plus)
extension is installed, this will automatically apply these settings, as well as overrides from
your own corresponding `.vscode/*.local.json` settings. If you don't install this extension, you can
use these as a reference for editing your own `.vscode/*.json` settings files. (See the extension
home page for more details).

## Debugging

Besides the usual debugging tools for Rust code and web sites, there is some logging in the code
that may be enabled to help troubleshoot. The `qsc` command-line compiler makes use of the Rust
crate [env_logger](https://docs.rs/env_logger/latest/env_logger/), which enables logging via
environment variables, for example `RUST_LOG=debug ./target/release/qsc ./samples/Grover.qs`.

## Citation

If you use Q#, Azure Quantum Development Kit, or Azure Quantum Resource Estimator, please cite as follows:

- Azure Quantum Development Kit:

```bibtex
@software{Microsoft_Azure_Quantum_Development,
   author = {{Microsoft}},
   license = {MIT},
   title = {{Azure Quantum Development Kit}},
   url = {https://github.com/microsoft/qsharp} }
```

- Q# programming language:

```bibtex
@inproceedings{Svore_2018, series={RWDSL2018},
   title={{Q\#: Enabling Scalable Quantum Computing and Development with a High-level DSL}},
   url={http://dx.doi.org/10.1145/3183895.3183901},
   DOI={10.1145/3183895.3183901},
   booktitle={Proceedings of the Real World Domain Specific Languages Workshop 2018},
   publisher={ACM},
   author={Svore, Krysta and Geller, Alan and Troyer, Matthias and Azariah, John and Granade, Christopher and Heim, Bettina and Kliuchnikov, Vadym and Mykhailova, Mariia and Paz, Andres and Roetteler, Martin},
   year={2018},
   month=feb, collection={RWDSL2018} }
```

- Azure Quantum Resource Estimator:

```bibtex
@inproceedings{Azure_Quantum_Resource_Estimator,
   author = {van Dam, Wim and Mykhailova, Mariia and Soeken, Mathias},
   title = {{Using Azure Quantum Resource Estimator for Assessing Performance of Fault Tolerant Quantum Computation}},
   year = {2023},
   isbn = {9798400707858},
   publisher = {Association for Computing Machinery},
   address = {New York, NY, USA},
   url = {https://doi.org/10.1145/3624062.3624211},
   doi = {10.1145/3624062.3624211},
   booktitle = {Proceedings of the SC '23 Workshops of The International Conference on High Performance Computing, Network, Storage, and Analysis},
   pages = {1414–1419},
   numpages = {6},
   series = {SC-W '23} }
```

## Feedback

If you have feedback about the content in this repository, please let us know by filing a [new issue](https://github.com/microsoft/qsharp/issues/new/choose)!

## Reporting Security Issues

Security issues and bugs should be reported privately following our [security issue documentation](./SECURITY.md#reporting-security-issues).

## Contributing

This project welcomes contributions and suggestions. Most contributions require you to agree to a
Contributor License Agreement (CLA) declaring that you have the right to, and actually do, grant us
the rights to use your contribution. For details, visit <https://cla.opensource.microsoft.com>.

When you submit a pull request, a CLA bot will automatically determine whether you need to provide
a CLA and decorate the PR appropriately (e.g., status check, comment). Simply follow the instructions
provided by the bot. You will only need to do this once across all repos using our CLA.

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or
contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

For more details, please see [CONTRIBUTING.md](./CONTRIBUTING.md).

## Legal and Licensing

### Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft
trademarks or logos is subject to and must follow
[Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general).
Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion
or imply Microsoft sponsorship. Any use of third-party trademarks or logos are subject to those
third-party's policies.
