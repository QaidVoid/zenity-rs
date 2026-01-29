use std::time::Instant;

/// A simple RAII timer that measures and logs elapsed time.
///
/// When dropped, it prints the elapsed time to stderr.
///
/// # Example
///
/// ```
/// use zenity_rs::timing::Timer;
///
/// {
///     let _timer = Timer::new("my_operation");
///     // ... do work ...
/// } // Timer drops here and logs elapsed time
/// ```
pub struct Timer {
    label: String,
    start: Instant,
}

impl Timer {
    /// Creates a new Timer with the given label.
    ///
    /// # Arguments
    ///
    /// * `label` - A string label describing the operation being timed
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        let start = Instant::now();
        Self {
            label,
            start,
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        eprintln!("{}: {:?}", self.label, duration);
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn test_timer_creation() {
        let _timer = Timer::new("test_operation");
        // Timer should be created successfully
        // The underscore ensures it's not dropped immediately
    }

    #[test]
    fn test_timer_measures_time() {
        // Start a timer
        let _timer = Timer::new("sleep_operation");
        // Sleep for a small duration
        thread::sleep(Duration::from_millis(10));
        // When _timer drops here, it should log the duration
        // We can't easily test the output, but we can ensure it compiles
    }

    #[test]
    fn test_timer_with_different_labels() {
        let _timer1 = Timer::new("operation_one");
        let _timer2 = Timer::new("operation_two");
        thread::sleep(Duration::from_millis(5));
        // Both timers should log their respective labels when dropped
    }

    #[test]
    fn test_timer_lifetime() {
        {
            let _timer = Timer::new("scoped_operation");
            thread::sleep(Duration::from_millis(5));
        } // Timer drops here and logs
        // Continue after timer has dropped
        thread::sleep(Duration::from_millis(5));
    }
}
