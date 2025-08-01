// Build script for cross-compilation support

fn main() {
    // Only run this for Android targets
    let target = std::env::var("TARGET").unwrap_or_default();
    
    if target.contains("android") {
        println!("cargo:warning=Building for Android target: {}", target);
        
        // Set up Android NDK environment if not already configured
        if std::env::var("ANDROID_NDK_ROOT").is_err() {
            // Common Android NDK locations
            let ndk_paths = [
                "/opt/android-ndk",
                "/usr/local/android-ndk", 
                "/opt/homebrew/share/android-ndk",
                "$HOME/Android/Sdk/ndk-bundle",
                "$HOME/android-ndk",
            ];
            
            for path in &ndk_paths {
                if std::path::Path::new(path).exists() {
                    println!("cargo:rustc-env=ANDROID_NDK_ROOT={}", path);
                    break;
                }
            }
        }
    }
}