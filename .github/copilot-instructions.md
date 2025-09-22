When reviewing pull requests, please consider the following:

1.  **Rust Best Practices:**
    *   Check for common Rust pitfalls, such as ownership and borrowing issues (e.g., unnecessary `.clone()`, fighting the borrow checker).
    *   Ensure the code is idiomatic Rust. For example, using iterators and their adapters over manual loops where appropriate.
    *   Verify that `Result` and `Option` are used correctly for error handling. Avoid using `unwrap()` or `expect()` on `Result` or `Option` types in production code.

2.  **Documentation:**
    *   Ensure that all new public functions, structs, enums, and traits have clear and concise documentation comments (`///`).
    *   The documentation should explain the purpose of the item, its parameters, and what it returns.

3.  **Code Style and Consistency:**
    *   The code should be formatted with `cargo fmt`.
    *   The code should be free of `cargo clippy` warnings, especially default lints.
    *   The style should be consistent with the surrounding code.

4.  **Performance:**
    *   Look for obvious performance issues, such as unnecessary allocations or inefficient algorithms.

5.  **Testing:**
    *   New functionality should be accompanied by unit tests.
    *   Existing tests should be updated if the underlying code has changed.
