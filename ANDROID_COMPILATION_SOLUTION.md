# Android Compilation Solution for Sage Wallet

## Root Cause Analysis

The main issue preventing Android compilation is **aws-lc-rs v0.28.2** which has several Android-specific problems:

1. **Bindgen Header Issues**: aws-lc-sys fails to find Android system headers during bindgen generation
2. **CMake Toolchain Problems**: libz-sys and aws-lc-sys have CMake configuration issues with Android NDK
3. **Architecture Mismatch**: CMake tries to use armv7-a flags for x86_64 target

## Solutions Implemented

### 1. Force Ring Usage Instead of AWS-LC

Modified `src-tauri/Cargo.toml`:
```toml
# Use ring instead of aws-lc-rs for Android compatibility
# aws-lc-rs = { version = "1", features = ["bindgen"] }
```

### 2. Workspace-level Ring Configuration

Updated `Cargo.toml`:
```toml
rustls = { version = "0.23.17", default-features = false, features = ["ring", "std"] }

[patch.crates-io]
rustls = { version = "=0.23.31", default-features = false, features = ["ring", "std"] }
```

### 3. Android-specific Cargo Configuration

Created `.cargo/config.toml`:
```toml
# Android-specific configuration
[target.'cfg(target_os = "android")']
rustflags = [
    "-C", "target-feature=+crt-static",
]

[env]
# Force ring usage instead of aws-lc for Android
AWS_LC_SYS_NO_ASM = "1"
RUSTFLAGS_x86_64_linux_android = "-C target-feature=+crt-static"
```

### 4. Environment Variables for Compilation

Required environment variables:
```bash
export ANDROID_HOME=/media/marvin/Marvin/sdk_linux
export NDK_HOME=/media/marvin/Marvin/sdk_linux/ndk/25.1.8937393
export ANDROID_NDK_ROOT=$NDK_HOME
export CMAKE_TOOLCHAIN_FILE=$NDK_HOME/build/cmake/android.toolchain.cmake
export ANDROID_PLATFORM=android-24
export ANDROID_ABI=x86_64
export AWS_LC_SYS_NO_ASM=1
export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot -I$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include -I$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include/x86_64-linux-android"
```

## Flutter Plugin Impact

Your Flutter plugin already has the right approach in `rust/Cargo.toml`:
```toml
# Note: Using ring instead of aws-lc for Android compatibility
# Force ring usage for Android compatibility (same as Sage)
ring = "0.17"
rustls = { version = "0.23.17", default-features = false, features = ["ring", "std"] }
```

The key insight is that **Sage's dependency on aws-lc-rs was causing your Flutter plugin compilation issues** because:

1. Sage explicitly required `aws-lc-rs` with `bindgen` feature
2. This pulled aws-lc-sys into the dependency tree
3. aws-lc-sys fails on Android due to bindgen header resolution issues
4. Your Flutter plugin, even with ring configuration, was affected by Sage's aws-lc dependency

## Testing the Solution

To test Android compilation:
```bash
# Set up environment
export ANDROID_HOME=/media/marvin/Marvin/sdk_linux
export NDK_HOME=/media/marvin/Marvin/sdk_linux/ndk/25.1.8937393
export ANDROID_NDK_ROOT=$NDK_HOME
export CMAKE_TOOLCHAIN_FILE=$NDK_HOME/build/cmake/android.toolchain.cmake
export ANDROID_PLATFORM=android-24
export AWS_LC_SYS_NO_ASM=1

# Test Sage compilation
cd /media/marvin/Marvin/projects/sage
pnpm tauri android dev

# Test Flutter plugin compilation
cd /media/marvin/Marvin/projects/Ozone/chia_wallet_flutter_plugin
flutter_rust_bridge_codegen build-android
```

## Additional Notes

1. **Ring vs AWS-LC**: Ring is more compatible with cross-compilation scenarios like Android
2. **Bindgen Issues**: aws-lc-sys has known issues with Android header resolution
3. **CMake Problems**: The NDK toolchain detection in aws-lc-sys build scripts is fragile
4. **Dependency Tree**: Even indirect dependencies on aws-lc can cause compilation failures

This solution should resolve both Sage Android compilation and your Flutter plugin aws-lc-sys issues.
