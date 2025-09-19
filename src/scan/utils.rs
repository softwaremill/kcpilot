/// Helper function to infer datacenter from hostname
pub fn infer_datacenter(hostname: &str) -> String {
    match () {
        _ if hostname.contains("-dc1-") => "dc1",
        _ if hostname.contains("-dc2-") => "dc2",
        _ if hostname.contains("-dc3-") => "dc3",
        _ => "unknown",
    }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_datacenter_dc1() {
        assert_eq!(infer_datacenter("kafka-dc1-broker1.example.com"), "dc1");
        assert_eq!(infer_datacenter("server-dc1-01"), "dc1");
    }

    #[test]
    fn test_infer_datacenter_dc2() {
        assert_eq!(infer_datacenter("kafka-dc2-broker1.example.com"), "dc2");
        assert_eq!(infer_datacenter("server-dc2-01"), "dc2");
    }

    #[test]
    fn test_infer_datacenter_dc3() {
        assert_eq!(infer_datacenter("kafka-dc3-broker1.example.com"), "dc3");
        assert_eq!(infer_datacenter("server-dc3-01"), "dc3");
    }

    #[test]
    fn test_infer_datacenter_unknown() {
        assert_eq!(infer_datacenter("kafka-broker1.example.com"), "unknown");
        assert_eq!(infer_datacenter("localhost"), "unknown");
        assert_eq!(infer_datacenter(""), "unknown");
        assert_eq!(infer_datacenter("kafka-prod-broker1"), "unknown");
    }

    #[test]
    fn test_infer_datacenter_case_sensitive() {
        // Should be case sensitive
        assert_eq!(infer_datacenter("kafka-DC1-broker1"), "unknown");
        assert_eq!(infer_datacenter("kafka-Dc1-broker1"), "unknown");
    }
}