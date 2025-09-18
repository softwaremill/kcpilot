pub mod scan;
pub mod analyze;
pub mod task;
pub mod ssh_test;

// Re-export handler functions for convenience
pub use scan::handle_scan_command;
pub use analyze::handle_analyze_command;
pub use task::handle_task_command;
pub use ssh_test::handle_ssh_test_command;