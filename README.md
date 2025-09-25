# The Tide Compiler

`Tide` is a research compiler that aims to _lower_ the backend-agnostic `Tide` intermediate representation (TIR) to various target-specific backends (e.g., LLVM) or native assembly.
The main goal of `Tide` is to provide a fast and adaptable compiler back-end framework that can be easily extended to support new target architectures and optimizations.
