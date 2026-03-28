/// Base path for all game assets served from the `dist/` directory.
/// Trunk copies the `assets/` folder to `dist/assets/` during `trunk build`,
/// making all files accessible via relative URLs from the WASM application.
pub const ASSETS_BASE: &str = "assets";

/// Returns a browser-relative URL for a named asset file.
pub fn asset_url(filename: &str) -> String {
    format!("{}/{}", ASSETS_BASE, filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    // RED → GREEN: asset base path is the correct relative directory name
    #[test]
    fn assets_base_is_relative_dir() {
        assert_eq!(ASSETS_BASE, "assets");
        // Must not start with '/' (absolute) or './' (Trunk serves relative to dist root)
        assert!(!ASSETS_BASE.starts_with('/'));
    }

    // RED → GREEN: asset_url produces correct relative URLs for .glb model files
    #[test]
    fn asset_url_for_glb_file() {
        assert_eq!(asset_url("unit.glb"), "assets/unit.glb");
    }

    // RED → GREEN: asset_url produces correct relative URLs for .png image files
    #[test]
    fn asset_url_for_png_file() {
        assert_eq!(asset_url("icon.png"), "assets/icon.png");
    }

    // RED → GREEN: asset_url correctly composes nested paths
    #[test]
    fn asset_url_for_nested_file() {
        assert_eq!(asset_url("models/circle.glb"), "assets/models/circle.glb");
    }
}
