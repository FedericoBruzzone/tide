pub mod idx;
pub mod index_slice;
pub mod index_vec;
mod variadic_log_macros; // to expose the macros `pub` is not needed

#[cfg(test)]
mod tests {
    #[test]
    fn test_library_available() {
        // This is a basic smoke test to ensure the library compiles and loads
        assert!(true);
    }
}
