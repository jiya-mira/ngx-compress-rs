//! Parent/child location-configuration inheritance.

use ngx::http::{Merge, MergeConfigError};

use super::CompressConfig;

impl Merge for CompressConfig {
    fn merge(&mut self, prev: &Self) -> Result<(), MergeConfigError> {
        merge_opt(&mut self.enable, prev.enable);
        merge_opt(&mut self.profile, prev.profile);
        merge_opt(&mut self.static_mode, prev.static_mode);
        merge_opt(&mut self.gzip, prev.gzip);
        merge_opt(&mut self.gzip_level, prev.gzip_level);
        merge_opt(&mut self.deflate, prev.deflate);
        merge_opt(&mut self.deflate_level, prev.deflate_level);
        merge_opt(&mut self.brotli, prev.brotli);
        merge_opt(&mut self.brotli_level, prev.brotli_level);
        merge_opt(&mut self.brotli_window, prev.brotli_window);
        merge_opt(&mut self.zstd, prev.zstd);
        merge_opt(&mut self.zstd_level, prev.zstd_level);
        merge_opt(&mut self.min_length, prev.min_length);
        merge_opt(&mut self.vary, prev.vary);
        merge_opt(&mut self.buffers, prev.buffers);
        if self.types.is_none() {
            self.types.clone_from(&prev.types);
        }
        Ok(())
    }
}

fn merge_opt<T: Copy>(child: &mut Option<T>, parent: Option<T>) {
    if child.is_none() {
        *child = parent;
    }
}
