//! # Utility Functions Module
//!
//! This module provides utility functions that improve code readability
//! and reduce boilerplate across the application.

/// Converts a vector of string-like items to Vec<String>.
/// 
/// This utility function accepts any iterable of items that can be converted
/// to String, eliminating repetitive `.to_string()` calls throughout the codebase.
/// 
/// # Generic Parameters
/// - `T`: Any type that implements `ToString`
/// - `I`: Any type that can be converted to an iterator over `T`
/// 
/// # Arguments
/// - `items`: An iterable of string-like items to convert
/// 
/// # Returns
/// - `Vec<String>`: A vector of owned strings
/// 
/// # Example
/// ```rust
/// use crate::utils::to_string_vec;
/// 
/// // Instead of:
/// let args = vec![
///     "--quality".to_string(),
///     "85".to_string(),
///     "--optimize".to_string(),
/// ];
/// 
/// // You can write:
/// let args = to_string_vec(["--quality", "85", "--optimize"]);
/// 
/// // Also works with mixed types:
/// let quality = 85;
/// let args = to_string_vec(["--quality", &quality.to_string(), "--optimize"]);
/// ```
pub fn to_string_vec<T, I>(items: I) -> Vec<String>
where
    T: ToString,
    I: IntoIterator<Item = T>,
{
    items.into_iter().map(|item| item.to_string()).collect()
}

/// Macro for even more convenient argument building.
/// 
/// This macro provides a convenient way to build argument vectors
/// without needing to import the function.
/// 
/// # Example
/// ```rust
/// use crate::args;
/// 
/// let quality = 85;
/// let args = args!["--quality", quality, "--optimize"];
/// ```
#[macro_export]
macro_rules! args {
    [$($item:expr),* $(,)?] => {
        $crate::utils::to_string_vec([$($item),*])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_string_vec_string_literals() {
        let result = to_string_vec(["hello", "world"]);
        assert_eq!(result, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn test_to_string_vec_mixed_types() {
        let num = 42;
        let result = to_string_vec(["--quality", &num.to_string(), "--optimize"]);
        assert_eq!(result, vec!["--quality".to_string(), "42".to_string(), "--optimize".to_string()]);
    }

    #[test]
    fn test_to_string_vec_empty() {
        let result: Vec<String> = to_string_vec(Vec::<&str>::new());
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_args_macro() {
        let quality = 85;
        let result = args!["--quality", quality, "--optimize"];
        assert_eq!(result, vec!["--quality".to_string(), "85".to_string(), "--optimize".to_string()]);
    }
}
