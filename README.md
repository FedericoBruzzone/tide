# The Tide Compiler

[![CI](https://github.com/FedericoBruzzone/tide/workflows/CI/badge.svg)](https://github.com/FedericoBruzzone/tide/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/FedericoBruzzone/tide#license)

`Tide` is a research compiler that uses its backend-agnostic intermediate representation (TIR) as a central abstraction. From the TIR, `Tide` is currently able to _lower_ it into existing backend-specific IRs (e.g., LLVM IR).

Future directions include the ability to (i) _lower_ TIR into other backend-specific IRs (e.g., WebAssembly), (ii) directly _generate machine_ code for target architectures (e.g., x86-64), or (iii) _interpret_ TIR for rapid prototyping and experimentation.
