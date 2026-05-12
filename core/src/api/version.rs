/// Public-API version. Mirrors the `/api/v1` URL prefix and the
/// `api_version` field of the `Info` response.
pub const API_VERSION: &str = "1";

/// Base path mounted at the root of every implementation.
pub const BASE_PATH: &str = "/api/v1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_path_includes_version() {
        assert!(BASE_PATH.ends_with(API_VERSION) || BASE_PATH.ends_with(&format!("v{API_VERSION}")));
    }
}
