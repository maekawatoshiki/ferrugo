#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            println!($($arg)*);
        }
    }
}

