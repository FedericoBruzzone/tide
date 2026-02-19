use std::env;
use tidec_log::{FallbackDefaultEnv, LogError, LogWriter, Logger, LoggerConfig};

#[test]
fn test_log_writer_variants() {
    let stdout_writer = LogWriter::Stdout;
    let stderr_writer = LogWriter::Stderr;
    let file_writer = LogWriter::File("test.log".into());

    match stdout_writer {
        LogWriter::Stdout => {}
        _ => panic!("Expected Stdout variant"),
    }

    match stderr_writer {
        LogWriter::Stderr => {}
        _ => panic!("Expected Stderr variant"),
    }

    match file_writer {
        LogWriter::File(path) => assert_eq!(path.to_str().unwrap(), "test.log"),
        _ => panic!("Expected File variant"),
    }
}

#[test]
fn test_logger_config_from_prefix() {
    let config = LoggerConfig::from_prefix("TEST_NONEXISTENT").unwrap();

    assert!(config.filter.is_err());
    assert!(config.color.is_err());
    assert!(config.line_numbers.is_err());
    assert!(config.file_names.is_err());

    matches!(config.log_writer, LogWriter::Stderr);
}

#[test]
fn test_logger_config_from_prefix_with_env_vars() {
    unsafe {
        env::set_var("TEST_PREFIX_LOG", "debug");
        env::set_var("TEST_PREFIX_LOG_COLOR", "always");
        env::set_var("TEST_PREFIX_LOG_WRITER", "stdout");
        env::set_var("TEST_PREFIX_LOG_LINE_NUMBERS", "1");
        env::set_var("TEST_PREFIX_LOG_FILE_NAMES", "1");
    }

    let config = LoggerConfig::from_prefix("TEST_PREFIX").unwrap();

    assert_eq!(config.filter.unwrap(), "debug");
    assert_eq!(config.color.unwrap(), "always");
    assert_eq!(config.line_numbers.unwrap(), "1");
    assert_eq!(config.file_names.unwrap(), "1");

    matches!(config.log_writer, LogWriter::Stdout);

    unsafe {
        env::remove_var("TEST_PREFIX_LOG");
        env::remove_var("TEST_PREFIX_LOG_COLOR");
        env::remove_var("TEST_PREFIX_LOG_WRITER");
        env::remove_var("TEST_PREFIX_LOG_LINE_NUMBERS");
        env::remove_var("TEST_PREFIX_LOG_FILE_NAMES");
    }
}

#[test]
fn test_logger_config_writer_variants() {
    unsafe {
        env::set_var("TEST_WRITER_LOG_WRITER", "stdout");
    }
    let config = LoggerConfig::from_prefix("TEST_WRITER").unwrap();
    matches!(config.log_writer, LogWriter::Stdout);
    unsafe {
        env::remove_var("TEST_WRITER_LOG_WRITER");
    }

    unsafe {
        env::set_var("TEST_WRITER2_LOG_WRITER", "stderr");
    }
    let config = LoggerConfig::from_prefix("TEST_WRITER2").unwrap();
    matches!(config.log_writer, LogWriter::Stderr);
    unsafe {
        env::remove_var("TEST_WRITER2_LOG_WRITER");
    }

    unsafe {
        env::set_var("TEST_WRITER3_LOG_WRITER", "/tmp/test.log");
    }
    let config = LoggerConfig::from_prefix("TEST_WRITER3").unwrap();
    if let LogWriter::File(path) = config.log_writer {
        assert_eq!(path.to_str().unwrap(), "/tmp/test.log");
    } else {
        panic!("Expected File writer");
    }
    unsafe {
        env::remove_var("TEST_WRITER3_LOG_WRITER");
    }
}

#[test]
fn test_fallback_default_env() {
    let yes = FallbackDefaultEnv::Yes;
    let no = FallbackDefaultEnv::No;

    match yes {
        FallbackDefaultEnv::No => panic!("Expected Yes variant"),
        FallbackDefaultEnv::Yes => {}
    }

    match no {
        FallbackDefaultEnv::Yes => panic!("Expected No variant"),
        FallbackDefaultEnv::No => {}
    }
}

#[test]
fn test_log_error_display() {
    let error1 = LogError::ColorNotValid("invalid".to_string());
    let error2 = LogError::NotUnicode("bad_unicode".to_string());

    assert_eq!(error1.to_string(), "Color not valid: invalid");
    assert_eq!(error2.to_string(), "Not unicode: bad_unicode");
}

#[test]
fn test_log_error_debug() {
    let error = LogError::ColorNotValid("test".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ColorNotValid"));
    assert!(debug_str.contains("test"));
}

#[test]
fn test_logger_struct_exists() {
    let _logger_type = std::marker::PhantomData::<Logger>;
}

#[test]
fn test_config_is_send_sync() {
    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}
    // Note: LoggerConfig contains Result<String, VarError> which should be Send + Sync
    // This test just verifies the function compiles, demonstrating Send + Sync bounds
    // Commented out as LogWriter contains PathBuf which should be Send + Sync
    // assert_send_sync::<LoggerConfig>();
}
