package com.mocharealm.accompanist.lyrics.text

/**
 * Native text engine for SDF glyph generation and text layout.
 * 
 * This engine uses Rust for high-performance text shaping (via rustybuzz)
 * and SDF generation. It maintains a glyph atlas with LRU cache eviction.
 * 
 * ## Usage
 * ```kotlin
 * val engine = NativeTextEngine()
 * engine.init(2048, 2048)  // Initialize with atlas size
 * engine.loadFont(fontBytes)  // Load primary font
 * 
 * // Process text and get layout
 * val layoutJson = engine.processText("Hello", 48f, 400f)
 * 
 * // Upload pending glyphs to atlas
 * if (engine.hasPendingUploads()) {
 *     val uploads = parsePendingUploads(engine.getPendingUploads())
 *     atlasManager.updateAtlas(uploads)
 * }
 * ```
 * 
 * ## SDF Processing
 * Glyph data is pre-processed in Rust with alpha values:
 * - A channel: normal text alpha (smoothstep at threshold 0.7)
 * - G channel: shadow alpha (smoothstep falloff for glow effect)
 */
expect class NativeTextEngine() {
    /**
     * Initializes the text engine with the given atlas dimensions.
     * Must be called before any other operations.
     * 
     * @param atlasWidth Width of the glyph atlas in pixels (typically 2048)
     * @param atlasHeight Height of the glyph atlas in pixels (typically 2048)
     */
    fun init(atlasWidth: Int, atlasHeight: Int)
    /**
     * Loads the primary font for text rendering.
     * 
     * @param bytes Raw bytes of the font file (TTF/OTF)
     */
    fun loadFont(bytes: ByteArray)
    /**
     * Loads a fallback font for missing glyphs.
     * Fallback fonts are tried in order when the primary font
     * doesn't contain a glyph.
     * 
     * @param bytes Raw bytes of the fallback font file (TTF/OTF)
     */
    fun loadFallbackFont(bytes: ByteArray)
    /**
     * Clears all loaded fallback fonts.
     */
    fun clearFallbackFonts()
    /**
     * Processes text and returns layout information as JSON.
     * This also generates SDF glyphs for any new characters.
     * 
     * @param text The text to layout
     * @param sizeFn Font size in pixels
     * @param weight Font weight (100-900, default 400)
     * @return JSON string containing layout result with glyph positions and atlas rects
     */
    fun processText(text: String, sizeFn: Float, weight: Float = 400f): String
    /**
     * Checks if there are pending glyph uploads.
     * @return true if new glyphs were generated and need to be uploaded to the atlas
     */
    fun hasPendingUploads(): Boolean
    /**
     * Gets pending glyph uploads as JSON.
     * Each upload contains position, size, and RGBA pixel data (pre-processed SDF).
     * 
     * @return JSON array of upload objects: [{x, y, width, height, data: "base64..."}]
     */
    fun getPendingUploads(): String
    /**
     * Gets the current atlas size.
     * @return JSON object: {width: N, height: N}
     */
    fun getAtlasSize(): String
    
    // Resource management
    /**
     * Releases all resources and clears the glyph cache.
     */
    fun destroy()
}
// Note: Zero-copy DirectByteBuffer API (processTextDirect, getPendingUploadsDirect) 
// is available only on JVM/Android platforms via platform-specific extensions.
