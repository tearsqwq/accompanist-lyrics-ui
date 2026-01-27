use crate::atlas::{AtlasManager, Rect};
use crate::font::FontWrapper;
use rustybuzz::{Face, UnicodeBuffer};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LayoutResult {
    pub glyph_count: usize,
    // Flat arrays for JNI transfer
    pub glyph_ids: Vec<u16>,
    pub positions: Vec<f32>,     // x, y interleaved (relative to baseline)
    pub atlas_rects: Vec<f32>,   // u, v, w, h in atlas
    pub glyph_offsets: Vec<f32>, // x_offset, y_offset interleaved (bearing from glyph origin to bitmap top-left)
    pub font_indices: Vec<u8>,   // Which font each glyph comes from (0 = primary, 1+ = fallback)
    pub total_width: f32,
    pub total_height: f32,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Clone)]
pub struct PendingUpload {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA data
}

/// A run of consecutive characters that share the same font.
struct TextRun {
    chars: Vec<char>,
    font_index: usize, // 0 = primary, 1+ = fallback
}

pub struct TextEngine {
    atlas: AtlasManager,
    // Primary font
    font: Option<FontWrapper>,
    font_data: Vec<u8>,
    // Fallback fonts (system fonts, etc.)
    fallback_fonts: Vec<FontWrapper>,
    fallback_font_data: Vec<Vec<u8>>,
    pending_uploads: Vec<PendingUpload>,
    pub atlas_width: u32,
    pub atlas_height: u32,
}

impl TextEngine {
    pub fn new(atlas_width: u32, atlas_height: u32) -> Self {
        Self {
            atlas: AtlasManager::new(atlas_width, atlas_height),
            font: None,
            font_data: Vec::new(),
            fallback_fonts: Vec::new(),
            fallback_font_data: Vec::new(),
            pending_uploads: Vec::new(),
            atlas_width,
            atlas_height,
        }
    }

    pub fn load_font(&mut self, font_bytes: Vec<u8>) {
        // Init FontWrapper for primary font
        info!("Loading PRIMARY font: {} bytes", font_bytes.len());
        if let Some(wrapper) = FontWrapper::from_bytes(&font_bytes, 0) {
            self.font = Some(wrapper);
            info!("PRIMARY font loaded successfully");
        } else {
            warn!("ERROR: Failed to load primary font!");
        }
        self.font_data = font_bytes;
    }

    /// Load a fallback font (e.g., system font for missing glyphs)
    pub fn load_fallback_font(&mut self, font_bytes: Vec<u8>) {
        let font_id = self.fallback_fonts.len() + 1; // 0 is primary
        info!(
            "Loading FALLBACK font #{}: {} bytes",
            font_id,
            font_bytes.len()
        );
        if let Some(wrapper) = FontWrapper::from_bytes(&font_bytes, font_id) {
            self.fallback_fonts.push(wrapper);
            self.fallback_font_data.push(font_bytes);
            info!(
                "FALLBACK font #{} loaded, total fallbacks: {}",
                font_id,
                self.fallback_fonts.len()
            );
        } else {
            warn!("ERROR: Failed to load fallback font #{}!", font_id);
        }
    }

    /// Load a fallback font from a file descriptor using memory mapping.
    /// This is more memory-efficient as it doesn't copy the entire font into RAM.
    /// The fd is duplicated internally so the caller can close it after this call.
    #[cfg(unix)]
    pub fn load_fallback_font_from_fd(&mut self, fd: i32) -> bool {
        use memmap2::Mmap;
        use std::os::unix::io::FromRawFd;

        let font_id = self.fallback_fonts.len() + 1;
        #[cfg(debug_assertions)]
        eprintln!(
            "[TextEngine] load_fallback_font_from_fd #{}: fd={}",
            font_id, fd
        );

        // Duplicate the FD so we own it
        let dup_fd = unsafe { libc::dup(fd) };
        if dup_fd < 0 {
            #[cfg(debug_assertions)]
            eprintln!("[TextEngine] Failed to dup fd!");
            return false;
        }

        // Create a File from the duplicated FD
        let file = unsafe { std::fs::File::from_raw_fd(dup_fd) };

        // Memory-map the file
        let mmap = match unsafe { Mmap::map(&file) } {
            Ok(m) => m,
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("[TextEngine] Failed to mmap: {:?}", e);
                return false;
            }
        };

        // Create FontWrapper from mmap
        if let Some(wrapper) = FontWrapper::from_mmap(mmap, font_id) {
            // We need to keep the font data reference for rustybuzz shaping
            // Since FontWrapper now owns the mmap, we store an empty vec as placeholder
            self.fallback_font_data.push(Vec::new());
            self.fallback_fonts.push(wrapper);
            #[cfg(debug_assertions)]
            eprintln!(
                "[TextEngine] Fallback font #{} loaded via mmap, total: {}",
                font_id,
                self.fallback_fonts.len()
            );
            true
        } else {
            #[cfg(debug_assertions)]
            eprintln!("[TextEngine] Failed to parse font from mmap!");
            false
        }
    }

    /// Clear all fallback fonts
    pub fn clear_fallback_fonts(&mut self) {
        self.fallback_fonts.clear();
        self.fallback_font_data.clear();
    }

    pub fn get_pending_uploads(&mut self) -> Vec<PendingUpload> {
        std::mem::take(&mut self.pending_uploads)
    }

    pub fn has_pending_uploads(&self) -> bool {
        !self.pending_uploads.is_empty()
    }

    pub fn get_atlas_size(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }

    /// Clear all cached data and reset the engine.
    /// Call this when switching fonts or to free memory.
    pub fn clear(&mut self) {
        self.atlas = AtlasManager::new(self.atlas_width, self.atlas_height);
        self.font = None;
        self.font_data = Vec::new();
        self.fallback_fonts.clear();
        self.fallback_font_data.clear();
        self.pending_uploads.clear();
    }

    pub fn process_text(&mut self, text: &str, size_px: f32, weight: f32) -> LayoutResult {
        let empty_result = LayoutResult {
            glyph_count: 0,
            glyph_ids: vec![],
            positions: vec![],
            atlas_rects: vec![],
            glyph_offsets: vec![],
            font_indices: vec![],
            total_width: 0.0,
            total_height: 0.0,
            ascent: 0.0,
            descent: 0.0,
        };

        if self.font_data.is_empty() {
            return empty_result;
        }

        let text_chars: Vec<char> = text.chars().collect();
        if text_chars.is_empty() {
            return empty_result;
        }

        // Quantize weight to reduce cache fragmentation (round to nearest 100)
        let weight_key = ((weight / 100.0).round() * 100.0) as u32;

        info!("========= PROCESSING TEXT =========");
        info!("Input: \"{}\" ({} chars)", text, text_chars.len());
        info!(
            "Font tower: 1 primary + {} fallbacks",
            self.fallback_fonts.len()
        );

        // ===========================================
        // Phase 1: Assign each character to a font
        // ===========================================
        let font_assignments = self.assign_fonts_to_chars(&text_chars);

        // ===========================================
        // Phase 2: Group into runs and shape each
        // ===========================================
        let runs = Self::group_into_runs(&text_chars, &font_assignments);

        info!("Grouped into {} runs", runs.len());

        let mut all_glyph_ids: Vec<u16> = Vec::new();
        let mut all_positions: Vec<f32> = Vec::new();
        let mut all_atlas_rects: Vec<f32> = Vec::new();
        let mut all_glyph_offsets: Vec<f32> = Vec::new();
        let mut all_font_indices: Vec<u8> = Vec::new();

        let mut x_cursor: f32 = 0.0;
        let mut max_ascent: f32 = 0.0;
        let mut max_descent: f32 = 0.0;
        let mut max_height: f32 = 0.0;

        for run in runs {
            let run_text: String = run.chars.iter().collect();
            let font_idx = run.font_index;

            let font_name = if font_idx == 0 {
                "PRIMARY".to_string()
            } else {
                format!("FALLBACK#{}", font_idx)
            };
            info!("Run: font={} text=\"{}\"", font_name, run_text);

            // Get font data for this run
            let font_data_ref: &[u8] = if font_idx == 0 {
                &self.font_data
            } else {
                let fb_idx = font_idx - 1;
                if fb_idx < self.fallback_fonts.len() {
                    &self.fallback_fonts[fb_idx].font_data
                } else {
                    continue;
                }
            };

            // Create Face for shaping
            let mut face = match Face::from_slice(font_data_ref, 0) {
                Some(f) => f,
                None => continue,
            };

            // Set font weight variation
            face.set_variations(&[rustybuzz::Variation {
                tag: rustybuzz::ttf_parser::Tag::from_bytes(b"wght"),
                value: weight,
            }]);

            // Shape the run with its own font
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(&run_text);
            let glyph_buffer = rustybuzz::shape(&face, &[], buffer);
            let glyph_infos = glyph_buffer.glyph_infos();
            let glyph_positions = glyph_buffer.glyph_positions();

            let units_per_em = face.units_per_em() as f32;
            let scale = size_px / units_per_em;

            // Update max metrics
            let run_ascent = face.ascender() as f32 * scale;
            let run_descent = face.descender() as f32 * scale;
            let run_height = face.height() as f32 * scale;
            if run_ascent > max_ascent {
                max_ascent = run_ascent;
            }
            if run_descent.abs() > max_descent.abs() {
                max_descent = run_descent;
            }
            if run_height > max_height {
                max_height = run_height;
            }

            for (info, gp) in glyph_infos.iter().zip(glyph_positions.iter()) {
                let glyph_id = info.glyph_id as u16;

                let glyph_info = if let Some(cached) = self.atlas.get_glyph_info_with_weight(
                    font_idx,
                    glyph_id,
                    size_px as u32,
                    weight_key,
                ) {
                    cached
                } else {
                    let sdf_result = if font_idx == 0 {
                        self.font
                            .as_mut()
                            .map(|f| f.generate_sdf(glyph_id, size_px, weight))
                    } else {
                        self.fallback_fonts
                            .get_mut(font_idx - 1)
                            .map(|f| f.generate_sdf(glyph_id, size_px, weight))
                    };

                    if let Some((bitmap, w, h, xmin, ymin)) = sdf_result {
                        if w > 0 && h > 0 {
                            if let Some(alloc_rect) = self.atlas.allocate(w, h) {
                                self.pending_uploads.push(PendingUpload {
                                    x: alloc_rect.x,
                                    y: alloc_rect.y,
                                    width: w,
                                    height: h,
                                    data: bitmap,
                                });
                                let info = crate::atlas::GlyphInfo {
                                    rect: alloc_rect,
                                    x_bearing: xmin,
                                    y_bearing: ymin,
                                    last_used: 0, // Will be set by cache_glyph_with_weight
                                };
                                self.atlas.cache_glyph_with_weight(
                                    font_idx,
                                    glyph_id,
                                    size_px as u32,
                                    weight_key,
                                    info,
                                );
                                info
                            } else {
                                crate::atlas::GlyphInfo {
                                    rect: Rect {
                                        x: 0,
                                        y: 0,
                                        width: 0,
                                        height: 0,
                                    },
                                    x_bearing: 0.0,
                                    y_bearing: 0.0,
                                    last_used: 0,
                                }
                            }
                        } else {
                            crate::atlas::GlyphInfo {
                                rect: Rect {
                                    x: 0,
                                    y: 0,
                                    width: 0,
                                    height: 0,
                                },
                                x_bearing: xmin,
                                y_bearing: ymin,
                                last_used: 0,
                            }
                        }
                    } else {
                        crate::atlas::GlyphInfo {
                            rect: Rect {
                                x: 0,
                                y: 0,
                                width: 0,
                                height: 0,
                            },
                            x_bearing: 0.0,
                            y_bearing: 0.0,
                            last_used: 0,
                        }
                    }
                };

                all_glyph_ids.push(glyph_id);
                all_font_indices.push(font_idx as u8);

                let x_pos = x_cursor + (gp.x_offset as f32 * scale);
                let y_pos = gp.y_offset as f32 * scale;
                all_positions.push(x_pos);
                all_positions.push(y_pos);

                all_glyph_offsets.push(glyph_info.x_bearing);
                all_glyph_offsets.push(glyph_info.y_bearing);

                x_cursor += gp.x_advance as f32 * scale;

                all_atlas_rects.push(glyph_info.rect.x as f32);
                all_atlas_rects.push(glyph_info.rect.y as f32);
                all_atlas_rects.push(glyph_info.rect.width as f32);
                all_atlas_rects.push(glyph_info.rect.height as f32);
            }
        }

        LayoutResult {
            glyph_count: all_glyph_ids.len(),
            glyph_ids: all_glyph_ids,
            positions: all_positions,
            atlas_rects: all_atlas_rects,
            glyph_offsets: all_glyph_offsets,
            font_indices: all_font_indices,
            total_width: x_cursor,
            total_height: max_height,
            ascent: max_ascent,
            descent: max_descent,
        }
    }

    /// Assign each character to a font based on glyph coverage.
    fn assign_fonts_to_chars(&self, chars: &[char]) -> Vec<usize> {
        let mut assignments = Vec::with_capacity(chars.len());
        let primary_face = Face::from_slice(&self.font_data, 0);
        let mut missing_chars: Vec<char> = Vec::new();

        for &ch in chars {
            // Try primary font first
            if let Some(ref face) = primary_face {
                if let Some(gid) = face.glyph_index(ch) {
                    if gid.0 != 0 {
                        assignments.push(0);
                        continue;
                    }
                }
            }

            // Try fallback fonts
            let mut assigned = 0usize;
            for (fb_idx, fb_wrapper) in self.fallback_fonts.iter().enumerate() {
                if let Some(fb_face) = Face::from_slice(&fb_wrapper.font_data, 0) {
                    if let Some(gid) = fb_face.glyph_index(ch) {
                        if gid.0 != 0 {
                            assigned = fb_idx + 1;
                            break;
                        }
                    }
                }
            }

            if assigned == 0 {
                // No font has this glyph!
                missing_chars.push(ch);
            }
            assignments.push(assigned);
        }

        if !missing_chars.is_empty() {
            warn!(
                "WARNING: {} chars have NO GLYPH in any font:",
                missing_chars.len()
            );
            for ch in &missing_chars {
                warn!("  - '{}' (U+{:04X})", ch, *ch as u32);
            }
        }

        assignments
    }

    /// Group consecutive characters with the same font assignment into runs.
    fn group_into_runs(chars: &[char], font_assignments: &[usize]) -> Vec<TextRun> {
        if chars.is_empty() {
            return Vec::new();
        }

        let mut runs = Vec::new();
        let mut current_font = font_assignments[0];
        let mut current_chars = vec![chars[0]];

        for i in 1..chars.len() {
            if font_assignments[i] == current_font {
                current_chars.push(chars[i]);
            } else {
                runs.push(TextRun {
                    chars: current_chars,
                    font_index: current_font,
                });
                current_font = font_assignments[i];
                current_chars = vec![chars[i]];
            }
        }
        runs.push(TextRun {
            chars: current_chars,
            font_index: current_font,
        });
        runs
    }
}
