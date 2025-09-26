# The Tide Compiler

`Tide` is a research compiler that uses its backend-agnostic intermediate representation (TIR) as a central abstraction. From TIR, `Tide` can (i) _lower_ into existing backend-specific IRs (e.g., LLVM IR), (ii) directly _generate machine_ code for target architectures (e.g., x86-64), or (iii) _interpret_ TIR for rapid prototyping and experimentation.

<!--
## `Tide` as a Compiler Framework

`Tide` is pluggable and extensible. 

## Analysis Passes

## Transformation Passes

## The TIR

The TIR is a low-level, strongly-typed, TAC-like intermediate representation.
-->