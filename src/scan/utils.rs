/// Helper function to infer datacenter from hostname
pub fn infer_datacenter(hostname: &str) -> String {
    match () {
        _ if hostname.contains("-dc1-") => "dc1",
        _ if hostname.contains("-dc2-") => "dc2",
        _ if hostname.contains("-dc3-") => "dc3",
        _ => "unknown",
    }.to_string()
}