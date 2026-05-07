/// Expands to [`MpvBundle`] macOS **`vf`** teardown helpers (must be invoked inside `impl MpvBundle`).
macro_rules! mpv_bundle_macos_vf_methods {
    () => {
        /// macOS only: pause **`CVDisplayLink`** / CALayer **`mpv`** draws during **`vf clr`**.
        #[cfg(target_os = "macos")]
        pub(crate) fn with_macos_vf_teardown<R>(&self, f: impl FnOnce() -> R) -> R {
            match self.macos.as_ref() {
                Some(m) => m.with_vf_teardown(f),
                None => f(),
            }
        }

        #[cfg(target_os = "macos")]
        pub(crate) fn macos_ping_render_context(&self) {
            if let Some(m) = self.macos.as_ref() {
                let _flags = m.ping_render_context();
            }
        }

        #[cfg(target_os = "macos")]
        pub(crate) fn macos_mark_display_pending(&self) {
            if let Some(m) = self.macos.as_ref() {
                m.mark_display_pending();
            }
        }
    };
}
