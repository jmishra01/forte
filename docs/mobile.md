# Building Forte for iOS and Android

The app is ready for mobile builds: the layout works on phone screens (the
note list and the note take turns), and desktop-only parts (self-update,
relaunch) are left out of mobile builds. What is still needed is the
platform toolchains and the generated native projects.

## iOS (needs a Mac)

1. Install Xcode from the App Store, open it once to finish setup, then:

   ```sh
   sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
   brew install cocoapods
   ```

2. Generate the Xcode project (once) and run in the simulator:

   ```sh
   npx tauri ios init
   npx tauri ios dev
   ```

3. To run on a phone or ship to the App Store, set your Apple team in
   `src-tauri/tauri.conf.json` under `bundle.iOS.developmentTeam`.

## Android

1. Install Android Studio. In its SDK Manager, install the Android SDK,
   "NDK (Side by side)" and "Android SDK Command-line Tools". Then set:

   ```sh
   export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
   export ANDROID_HOME="$HOME/Library/Android/sdk"
   export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk | tail -1)"
   rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
   ```

2. Generate the Android project (once) and run on an emulator or device:

   ```sh
   npx tauri android init
   npx tauri android dev
   ```

## Known differences on phones

- Notes live in the app's own storage. Choosing another folder is limited by
  what each platform's file picker allows.
- Git sync needs the `git` program, which phones don't have; the sync button
  doesn't appear there. Use a synced folder or Git on a computer instead.
- Updates come through the App Store / Play Store, not the built-in updater.
