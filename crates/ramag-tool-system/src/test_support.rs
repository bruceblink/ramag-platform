//! Test-only assertion helpers that keep the workspace Clippy contract explicit.

/// Extract a successful value while retaining a useful panic when a test fixture is invalid.
pub(crate) trait TestUnwrapExt {
    type Output;

    fn test_unwrap(self) -> Self::Output;
    fn test_unwrap_msg(self, message: &str) -> Self::Output;
}

impl<T> TestUnwrapExt for Option<T> {
    type Output = T;

    fn test_unwrap(self) -> Self::Output {
        self.test_unwrap_msg("expected an optional test value")
    }

    fn test_unwrap_msg(self, message: &str) -> Self::Output {
        match self {
            Some(value) => value,
            None => std::panic::resume_unwind(Box::new(message.to_owned())),
        }
    }
}

impl<T, E> TestUnwrapExt for Result<T, E> {
    type Output = T;

    fn test_unwrap(self) -> Self::Output {
        self.test_unwrap_msg("expected a successful test result")
    }

    fn test_unwrap_msg(self, message: &str) -> Self::Output {
        match self {
            Ok(value) => value,
            Err(_) => std::panic::resume_unwind(Box::new(message.to_owned())),
        }
    }
}

/// Extract an expected error without using `unwrap_err`, which is denied by workspace Clippy.
pub(crate) trait TestUnwrapErrExt {
    type Output;

    fn test_unwrap_err(self) -> Self::Output;
}

impl<T, E> TestUnwrapErrExt for Result<T, E> {
    type Output = E;

    fn test_unwrap_err(self) -> Self::Output {
        match self {
            Err(error) => error,
            Ok(_) => {
                std::panic::resume_unwind(Box::new("expected a failed test result".to_owned()))
            }
        }
    }
}
