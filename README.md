# Runtime Scripting for Rust Applications

This repository provides a demo for embedding JavaScript or TypeScript for the purpose of runtime scripting
into a Rust application.
It was created as a concise but complete example for my [Runtime Scripting for Rust Applications](Runtime Scripting for Rust Applications.pdf) talk at EuroRust 2024 in Vienna.
To this end, the [Deno](https://deno.com/) runtime is used to provide a batteries-included scripting environment.
Deno provides certain [security mechanisms](https://docs.deno.com/runtime/fundamentals/security/) to
restrict access to the network, file system, and other functionalities.
These can be configured as part of the runtime worker.

The demo can be run as `cargo run -- <script_file> [<function_name>]`, where the optional `function_name` defaults to the scripts `default` export.

