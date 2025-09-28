# The Tide Compiler

[![CI](https://github.com/FedericoBruzzone/tide/workflows/CI/badge.svg)](https://github.com/FedericoBruzzone/tide/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/FedericoBruzzone/tide#license)

`Tide` is a research compiler that uses its backend-agnostic intermediate representation (TIR) as a central abstraction. From TIR, `Tide` can (i) _lower_ into existing backend-specific IRs (e.g., LLVM IR), (ii) directly _generate machine_ code for target architectures (e.g., x86-64), or (iii) _interpret_ TIR for rapid prototyping and experimentation.

<!--
## `Tide` as a Compiler Framework

`Tide` is pluggable and extensible. 

## Analysis Passes

## Transformation Passes

## The TIR

The TIR is a low-level, strongly-typed, TAC-like intermediate representation.
-->