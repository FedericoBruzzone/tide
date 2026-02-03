//! Integration test utilities for the Tide compiler.
//!
//! This crate provides shared infrastructure for end-to-end compilation tests.
//!
//! # Running the tests
//!
//! These tests must be run with a single thread to avoid race conditions
//! caused by changing the current working directory:
//!
//! ```sh
//! cargo test -p tide_tests -- --test-threads=1
//! ```

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use tidec_abi::target::{BackendKind, TirTarget};
use tidec_codegen_llvm::entry::llvm_codegen_lir_unit;
use tidec_tir::body::TirUnit;
use tidec_tir::ctx::{EmitKind, TirArena, TirArgs, TirCtx};

/// Global mutex to serialize tests that change the current directory.
pub static TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Helper struct for running integration tests.
pub struct TestRunner {
    /// Directory for test artifacts.
    test_dir: PathBuf,
    /// Name of the test (used for file naming).
    test_name: String,
}

impl TestRunner {
    /// Create a new test runner with a unique test directory.
    pub fn new(test_name: &str) -> Self {
        // Use a unique ID to avoid race conditions when tests run in parallel
        let unique_id = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_dir = std::env::temp_dir().join(format!(
            "tidec_test_{}_{}_{}",
            test_name, unique_id, timestamp
        ));
        std::fs::create_dir_all(&test_dir).expect("Failed to create test directory");

        Self {
            test_dir,
            test_name: test_name.to_string(),
        }
    }

    /// Get the path for the object file.
    /// Note: The codegen always produces "main.o" based on the module name.
    pub fn object_path(&self) -> PathBuf {
        self.test_dir.join("main.o")
    }

    /// Get the path for the executable.
    pub fn executable_path(&self) -> PathBuf {
        self.test_dir.join(&self.test_name)
    }

    /// Compile TIR to an object file.
    ///
    /// Note: This acquires a global mutex because changing the current directory
    /// affects all threads in the process.
    pub fn compile<'a>(&self, tir_ctx: TirCtx<'a>, tir_unit: TirUnit<'a>) {
        // Acquire the mutex to prevent concurrent directory changes
        let _guard = TEST_MUTEX.lock().expect("Failed to acquire test mutex");

        // Change to test directory so the object file is written there
        let original_dir = std::env::current_dir().expect("Failed to get current directory");
        std::env::set_current_dir(&self.test_dir).expect("Failed to change to test directory");

        // Compile
        llvm_codegen_lir_unit(tir_ctx, tir_unit);

        // Change back
        std::env::set_current_dir(original_dir).expect("Failed to restore directory");

        // Verify the object file was created
        assert!(
            self.object_path().exists(),
            "Object file was not created at {:?}",
            self.object_path()
        );
    }

    /// Link the object file into an executable.
    pub fn link(&self) -> Result<(), String> {
        // Check that object file exists
        if !self.object_path().exists() {
            return Err(format!(
                "Object file does not exist: {:?}",
                self.object_path()
            ));
        }

        let output = if cfg!(target_os = "macos") {
            // On macOS, we need to link with the system SDK
            let sdk_path = Command::new("xcrun")
                .args(["--show-sdk-path"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();

            Command::new("ld")
                .args([
                    self.object_path().to_str().unwrap(),
                    "-o",
                    self.executable_path().to_str().unwrap(),
                    "-lSystem",
                    "-syslibroot",
                    &sdk_path,
                ])
                .output()
        } else {
            // On Linux/other, use cc
            Command::new("cc")
                .args([
                    self.object_path().to_str().unwrap(),
                    "-o",
                    self.executable_path().to_str().unwrap(),
                ])
                .output()
        };

        match output {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(format!(
                "Linker failed with status {:?}\nstderr: {}",
                o.status,
                String::from_utf8_lossy(&o.stderr)
            )),
            Err(e) => Err(format!("Failed to run linker: {}", e)),
        }
    }

    /// Run the executable and return the exit code.
    pub fn run(&self) -> Option<i32> {
        Command::new(self.executable_path())
            .output()
            .ok()
            .and_then(|o| o.status.code())
    }

    /// Run the executable and return stdout as a string.
    pub fn run_with_output(&self) -> Option<(i32, String)> {
        Command::new(self.executable_path()).output().ok().map(|o| {
            (
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stdout).to_string(),
            )
        })
    }
}

impl Drop for TestRunner {
    fn drop(&mut self) {
        // Clean up test artifacts
        let _ = std::fs::remove_dir_all(&self.test_dir);
    }
}

/// Create a TIR context for testing with the default LLVM backend.
pub struct TestContext<'ctx> {
    pub target: TirTarget,
    pub arguments: TirArgs,
    pub arena: TirArena<'ctx>,
}

impl<'ctx> TestContext<'ctx> {
    /// Create a new test context.
    pub fn new() -> Self {
        Self {
            target: TirTarget::new(BackendKind::Llvm),
            arguments: TirArgs {
                emit_kind: EmitKind::Object,
            },
            arena: TirArena::default(),
        }
    }
}

impl Default for TestContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}
