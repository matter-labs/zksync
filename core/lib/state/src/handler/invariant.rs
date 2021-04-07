macro_rules! invariant {
    ($condition:expr,$error_class:expr) => {{
        if !$condition {
            return Err($error_class);
        }
    }};
}
