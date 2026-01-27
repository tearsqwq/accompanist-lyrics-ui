package com.mocharealm.accompanist.lyrics.ui.composable.lyrics

import androidx.compose.runtime.Stable
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntSize
import com.mocharealm.accompanist.lyrics.core.model.karaoke.KaraokeSyllable
import com.mocharealm.accompanist.lyrics.text.NativeTextEngine
import com.mocharealm.accompanist.lyrics.ui.utils.isArabic
import com.mocharealm.accompanist.lyrics.ui.utils.isDevanagari
import com.mocharealm.accompanist.lyrics.ui.utils.isPunctuation
import com.mocharealm.accompanist.lyrics.ui.utils.isPureCjk
import kotlin.math.pow

/**
 * Represents the raw layout result from the native text engine (Rust).
 * Contains positioning and sizing information for a block of text.
 *
 * @param glyph_count Number of glyphs in this result.
 * @param glyph_ids List of glyph IDs (indices in the font).
 * @param positions List of positions (x, y) relative to the baseline.
 * @param atlas_rects List of atlas coordinates (x, y, w, h) for each glyph.
 * @param glyph_offsets List of bearing offsets (x, y) from glyph origin.
 * @param total_width Total width of the text block.
 * @param total_height Total height (bounding box height).
 * @param ascent Font ascent (distance from baseline to top).
 * @param descent Font descent (distance from baseline to bottom).
 */
@Stable
data class NativeLayoutResult(
    val glyph_count: Int,
    val glyph_ids: List<Int>,
    val positions: List<Float>,
    val atlas_rects: List<Float>,
    val glyph_offsets: List<Float>, // x_offset, y_offset interleaved (bearing from glyph origin to bitmap)
    val total_width: Float,
    val total_height: Float,
    val ascent: Float,
    val descent: Float
) {
    val size: IntSize get() = IntSize(total_width.toInt(), total_height.toInt())
    val firstBaseline: Float get() = ascent
}

// Helper to parse JSON (MVP hack: use regex or assume strict format from Rust)
// Robust way: kotlinx.serialization.
// For now, I will use a dummy function that needs implementation or manual parsing.
fun parseRustResult(json: String): NativeLayoutResult {
    // Parse JSON format: {"glyph_count":N,"glyph_ids":[...],"positions":[...],"atlas_rects":[...],"glyph_offsets":[...],"total_width":F,...}
    if (json.isEmpty() || json == "{}") {
        return NativeLayoutResult(0, emptyList(), emptyList(), emptyList(), emptyList(), 0f, 0f, 0f, 0f)
    }
    
    try {
        // Extract numeric fields using regex
        val glyphCountMatch = Regex(""""glyph_count"\s*:\s*(\d+)""").find(json)
        val totalWidthMatch = Regex(""""total_width"\s*:\s*([\d.]+)""").find(json)
        val totalHeightMatch = Regex(""""total_height"\s*:\s*([\d.]+)""").find(json)
        val ascentMatch = Regex(""""ascent"\s*:\s*([\d.]+)""").find(json)
        val descentMatch = Regex(""""descent"\s*:\s*([\d.-]+)""").find(json)
        
        val glyphCount = glyphCountMatch?.groupValues?.get(1)?.toIntOrNull() ?: 0
        val totalWidth = totalWidthMatch?.groupValues?.get(1)?.toFloatOrNull() ?: 0f
        val totalHeight = totalHeightMatch?.groupValues?.get(1)?.toFloatOrNull() ?: 0f
        val ascent = ascentMatch?.groupValues?.get(1)?.toFloatOrNull() ?: 0f
        val descent = descentMatch?.groupValues?.get(1)?.toFloatOrNull() ?: 0f
        
        // Extract arrays
        val glyphIdsMatch = Regex(""""glyph_ids"\s*:\s*\[([\d,\s]*)\]""").find(json)
        val positionsMatch = Regex(""""positions"\s*:\s*\[([\d.,\s-]*)\]""").find(json)
        val atlasRectsMatch = Regex(""""atlas_rects"\s*:\s*\[([\d.,\s-]*)\]""").find(json)
        val glyphOffsetsMatch = Regex(""""glyph_offsets"\s*:\s*\[([\d.,\s-]*)\]""").find(json)
        
        val glyphIds = glyphIdsMatch?.groupValues?.get(1)
            ?.split(",")
            ?.mapNotNull { it.trim().toIntOrNull() }
            ?: emptyList()
        
        val positions = positionsMatch?.groupValues?.get(1)
            ?.split(",")
            ?.mapNotNull { it.trim().toFloatOrNull() }
            ?: emptyList()
        
        val atlasRects = atlasRectsMatch?.groupValues?.get(1)
            ?.split(",")
            ?.mapNotNull { it.trim().toFloatOrNull() }
            ?: emptyList()
        
        val glyphOffsets = glyphOffsetsMatch?.groupValues?.get(1)
            ?.split(",")
            ?.mapNotNull { it.trim().toFloatOrNull() }
            ?: emptyList()
        
        return NativeLayoutResult(
            glyph_count = glyphCount,
            glyph_ids = glyphIds,
            positions = positions,
            atlas_rects = atlasRects,
            glyph_offsets = glyphOffsets,
            total_width = totalWidth,
            total_height = totalHeight,
            ascent = ascent,
            descent = descent
        )
    } catch (e: Exception) {
        // Fallback to empty on parse error
        return NativeLayoutResult(0, emptyList(), emptyList(), emptyList(), emptyList(), 0f, 0f, 0f, 0f)
    }
}
/**
 * Represents the layout information for a single karaoke syllable.
 * This includes the text layout from the native engine, as well as animation metadata.
 *
 * @param syllable The original karaoke syllable data.
 * @param layoutResult The native text layout result for this syllable.
 * @param wordId ID grouping syllables into words (for word-level wrapping).
 * @param useAwesomeAnimation Whether this syllable uses complex per-character animations (bounce, swell).
 * @param width Total width of the syllable.
 * @param position Layout position relative to the line/row.
 * @param wordPivot Pivot point for word-level transformations.
 * @param wordAnimInfo Metadata for word-level animations.
 * @param charOffsetInWord Constant offset of this syllable's characters within the word.
 * @param charLayouts Individual character layouts (for complex animations).
 * @param charOriginalBounds Bounds of individual characters relative to the syllable.
 * @param firstBaseline Baseline offset from the top.
 * @param floatEndingTime Calculated time when the "float" animation should settle.
 */
@Stable
data class SyllableLayout(
    val syllable: KaraokeSyllable,
    val layoutResult: NativeLayoutResult, // Replaced
    val wordId: Int,
    val useAwesomeAnimation: Boolean,
    val width: Float = layoutResult.total_width,
    val position: Offset = Offset.Zero,
    val wordPivot: Offset = Offset.Zero,
    val wordAnimInfo: WordAnimationInfo? = null,
    val charOffsetInWord: Int = 0,
    val charLayouts: List<NativeLayoutResult>? = null, // Replaced
    val charOriginalBounds: List<Rect>? = null,
    val firstBaseline: Float = layoutResult.firstBaseline,
    val floatEndingTime: Long = 0L 
)

@Stable
data class WordAnimationInfo(
    val wordStartTime: Long,
    val wordEndTime: Long,
    val wordContent: String,
    val wordDuration: Long = wordEndTime - wordStartTime
)

@Stable
data class WrappedLine(
    val syllables: List<SyllableLayout>, val totalWidth: Float
)

fun String.shouldUseSimpleAnimation(): Boolean {
    val cleanedStr = this.filter { !it.isWhitespace() && !it.toString().isPunctuation() }
    if (cleanedStr.isEmpty()) return false
    return cleanedStr.isPureCjk() || cleanedStr.any { it.isArabic() || it.isDevanagari() }
}

fun groupIntoWords(syllables: List<KaraokeSyllable>): List<List<KaraokeSyllable>> {
    if (syllables.isEmpty()) return emptyList()
    val words = mutableListOf<List<KaraokeSyllable>>()
    var currentWord = mutableListOf<KaraokeSyllable>()
    syllables.forEach { syllable ->
        currentWord.add(syllable)
        if (syllable.content.trimEnd().length < syllable.content.length) {
            words.add(currentWord.toList())
            currentWord = mutableListOf()
        }
    }
    if (currentWord.isNotEmpty()) {
        words.add(currentWord.toList())
    }
    return words
}

/**
 * Measures all syllables in a line using the native text engine and determines
 * appropriate animation strategies (simple fade/slide vs. complex bounce/swell).
 *
 * It calculates:
 * - Text layout for each syllable
 * - Whether "Awesome" (complex) animation should be used based on duration and language.
 * - Animation timing constraints.
 *
 * @param syllables List of syllables to measure.
 * @param textMeasurer Helper for measuring constraint spaces (space char).
 * @param style Text style to use.
 * @param isAccompanimentLine Whether this is a backing vocal line.
 * @param spaceWidth Pre-measured width of a space character.
 * @param fontFamilyResolver Resolver for fonts.
 * @param density Screen density.
 * @param nativeEngine The native text engine for layout.
 * @param platformContext Platform context for font loading.
 * @return List of [SyllableLayout] with measurement and animation data.
 */
fun measureSyllablesAndDetermineAnimation(
    syllables: List<KaraokeSyllable>,
    textMeasurer: TextMeasurer,
    style: TextStyle,
    isAccompanimentLine: Boolean,
    spaceWidth: Float,
    fontFamilyResolver: FontFamily.Resolver,
    density: Density,
    nativeEngine: NativeTextEngine,
    platformContext: Any? = null 
): List<SyllableLayout> {

    val words = groupIntoWords(syllables)
    val fastCharAnimationThresholdMs = 200f

    val initialLayouts = words.flatMapIndexed { wordIndex, word ->
        val wordContent = word.joinToString("") { it.content }
        val wordDuration = if (word.isNotEmpty()) word.last().end - word.first().start else 0
        val perCharDuration = if (wordContent.isNotEmpty() && wordDuration > 0) {
            wordDuration.toFloat() / wordContent.length
        } else {
            0f
        }

        val useAwesomeAnimation =
            perCharDuration > fastCharAnimationThresholdMs && wordDuration >= 1000
                    && !wordContent.shouldUseSimpleAnimation()
                    && !isAccompanimentLine

        word.map { syllable ->
            // NATIVE ENGINE CALL
            // Font size? style.fontSize. Assuming pixel size is needed.
            // style.fontSize.value needs density.
            val densityVal = density.density
            val fontSizePx = if (style.fontSize.isSp) style.fontSize.value * density.fontScale * density.density else style.fontSize.value
            val fontWeight = style.fontWeight?.weight?.toFloat() ?: 400f
            
            val jsonResult = nativeEngine.processText(syllable.content, fontSizePx, fontWeight)
            var layoutResult = parseRustResult(jsonResult)

            // --- Fix: 修正尾部空格宽度丢失 ---
            var layoutWidth = layoutResult.total_width
            if (syllable.content.endsWith(" ")) {
                 val trimmedJson = nativeEngine.processText(syllable.content.trimEnd(), fontSizePx, fontWeight)
                 val trimmedLayout = parseRustResult(trimmedJson)
                 val trimmedWidth = trimmedLayout.total_width
                 
                if (layoutWidth <= trimmedWidth) {
                    val spaceCount = syllable.content.length - syllable.content.trimEnd().length
                    layoutWidth = trimmedWidth + (spaceWidth * spaceCount)
                    // Update layoutResult width
                    layoutResult = layoutResult.copy(total_width = layoutWidth)
                }
            }
            // -----------------------------

            // 新增：如果需要高级动画，预先测量每个字符
            val (charLayouts, charBounds) = if (useAwesomeAnimation) {
                val layouts = syllable.content.map { char ->
                     val cJson = nativeEngine.processText(char.toString(), fontSizePx, fontWeight)
                     parseRustResult(cJson)
                }
                // Calculate bounds from layouts
                var xOffset = 0f
                val bounds = layouts.map { layout ->
                    val rect = Rect(xOffset, 0f, xOffset + layout.total_width, layout.total_height)
                    xOffset += layout.total_width
                    rect
                }
                Pair(layouts, bounds)
            } else {
                Pair(null, null)
            }

            SyllableLayout(
                syllable = syllable,
                layoutResult = layoutResult,
                wordId = wordIndex,
                useAwesomeAnimation = useAwesomeAnimation,
                width = layoutWidth, // 使用修正后的宽度
                charLayouts = charLayouts,      // 存入缓存
                charOriginalBounds = charBounds,
                firstBaseline = layoutResult.firstBaseline
            )
        }
    }

    // --- Post-processing: Calculate floatEndingTime ---
    val n = initialLayouts.size
    if (n == 0) return emptyList()

    val floatEndTimes = LongArray(n)
    val lineEndTime = initialLayouts.last().syllable.end.toLong()

    // 1. Initial Pass: Set based on self-properties and next-is-awesome constraints
    for (i in 0 until n) {
        val layout = initialLayouts[i]
        // Default: start + 700ms, constrained by line end
        var endTime = (layout.syllable.start + 700).coerceAtMost(lineEndTime.toInt()).toLong()

        // Constraint: If next syllable is Awesome, this one must finish before next starts
        if (i + 1 < n) {
            val nextLayout = initialLayouts[i + 1]
            if (nextLayout.useAwesomeAnimation) {
                // Determine effective start time of the next awesome animation
                // For awesome animation, the word starts, but characters animate based on internal timing.
                // Safest rule: Finish before the word start time.
                val nextStartTime = nextLayout.syllable.start.toLong()
                if (endTime > nextStartTime) {
                    endTime = nextStartTime
                }
            }
        }
        floatEndTimes[i] = endTime
    }

    // 2. Backward Pass: Propagate constraints for non-awesome predecessors
    // If layout[i] is effectively constrained by layout[i+1], then layout[i] needs to finish no later than layout[i+1]
    // ONLY if layout[i+1] itself is a standard float animation.
    // If layout[i+1] is awesome, we already handled it in Pass 1.
    for (i in n - 2 downTo 0) {
        val nextLayout = initialLayouts[i + 1]
        if (!nextLayout.useAwesomeAnimation) {
             if (floatEndTimes[i] > floatEndTimes[i+1]) {
                 floatEndTimes[i] = floatEndTimes[i+1]
             }
        }
    }

    return initialLayouts.mapIndexed { index, layout ->
        layout.copy(floatEndingTime = floatEndTimes[index])
    }
}

fun calculateGreedyWrappedLines(
    syllableLayouts: List<SyllableLayout>,
    availableWidthPx: Float,
    nativeEngine: NativeTextEngine,
    style: TextStyle,
    density: Density
): List<WrappedLine> {
    val lines = mutableListOf<WrappedLine>()
    val currentLine = mutableListOf<SyllableLayout>()
    var currentLineWidth = 0f

    val wordGroups = mutableListOf<List<SyllableLayout>>()
    if (syllableLayouts.isNotEmpty()) {
        var currentWordGroup = mutableListOf<SyllableLayout>()
        var currentWordId = syllableLayouts.first().wordId

        syllableLayouts.forEach { layout ->
            if (layout.wordId != currentWordId) {
                wordGroups.add(currentWordGroup)
                currentWordGroup = mutableListOf()
                currentWordId = layout.wordId
            }
            currentWordGroup.add(layout)
        }
        wordGroups.add(currentWordGroup)
    }

    // 按单词进行排版
    wordGroups.forEach { wordSyllables ->
        val wordWidth = wordSyllables.sumOf { it.width.toDouble() }.toFloat()

        // 如果当前行能放下整个单词
        if (currentLineWidth + wordWidth <= availableWidthPx) {
            currentLine.addAll(wordSyllables)
            currentLineWidth += wordWidth
        } else {
            // 放不下，先换行（如果当前行不是空的）
            if (currentLine.isNotEmpty()) {
                val trimmedDisplayLine =
                    trimDisplayLineTrailingSpaces(currentLine, nativeEngine, style, density)
                if (trimmedDisplayLine.syllables.isNotEmpty()) {
                    lines.add(trimmedDisplayLine)
                }
                currentLine.clear()
                currentLineWidth = 0f
            }

            // 换行后，检查新行能不能放下这个单词
            if (wordWidth <= availableWidthPx) {
                // 能放下，直接加入
                currentLine.addAll(wordSyllables)
                currentLineWidth += wordWidth
            } else {
                // 特殊情况: 单词超级长（比如德语符合词），比一整行还宽
                // 此时必须破坏单词完整性，退化回按音节换行
                wordSyllables.forEach { syllable ->
                    if (currentLineWidth + syllable.width > availableWidthPx && currentLine.isNotEmpty()) {
                        val trimmedLine =
                            trimDisplayLineTrailingSpaces(currentLine, nativeEngine, style, density)
                        if (trimmedLine.syllables.isNotEmpty()) lines.add(trimmedLine)
                        currentLine.clear()
                        currentLineWidth = 0f
                    }
                    currentLine.add(syllable)
                    currentLineWidth += syllable.width
                }
            }
        }
    }

    // 处理最后一行
    if (currentLine.isNotEmpty()) {
        val trimmedDisplayLine = trimDisplayLineTrailingSpaces(currentLine, nativeEngine, style, density)
        if (trimmedDisplayLine.syllables.isNotEmpty()) {
            lines.add(trimmedDisplayLine)
        }
    }
    return lines
}

/**
 * Calculates line wrapping for syllables using a balanced (minimum raggedness) algorithm
 * (Knuth-Plass inspired dynamic programming approach).
 *
 * Tries to distribute words evenly across lines to minimize empty space at the end of lines.
 * Falls back to greedy wrapping if optimization fails to find a valid solution.
 *
 * @param syllableLayouts List of pre-measured syllable layouts.
 * @param availableWidthPx Maximum width available for a line.
 * @param nativeEngine Native engine (used for trimming/re-measuring if needed).
 * @param style Text style.
 * @param density Screen density.
 * @return List of [WrappedLine] representing the broken lines.
 */
fun calculateBalancedLines(
    syllableLayouts: List<SyllableLayout>,
    availableWidthPx: Float,
    nativeEngine: NativeTextEngine,
    style: TextStyle,
    density: Density
): List<WrappedLine> {
    if (syllableLayouts.isEmpty()) return emptyList()

    val n = syllableLayouts.size
    val costs = DoubleArray(n + 1) { Double.POSITIVE_INFINITY }
    val breaks = IntArray(n + 1)
    costs[0] = 0.0

    for (i in 1..n) {
        var currentLineWidth = 0f
        for (j in i downTo 1) {
            if (j > 1 && syllableLayouts[j - 2].wordId == syllableLayouts[j - 1].wordId) {
                currentLineWidth += syllableLayouts[j - 1].width
                if (currentLineWidth > availableWidthPx) break
                continue
            }

            currentLineWidth += syllableLayouts[j - 1].width

            if (currentLineWidth > availableWidthPx) break

            val badness = (availableWidthPx - currentLineWidth).pow(2).toDouble()

            if (costs[j - 1] != Double.POSITIVE_INFINITY && costs[j - 1] + badness < costs[i]) {
                costs[i] = costs[j - 1] + badness
                breaks[i] = j - 1
            }
        }
    }

    if (costs[n] == Double.POSITIVE_INFINITY) {
        return calculateGreedyWrappedLines(syllableLayouts, availableWidthPx, nativeEngine, style, density)
    }

    val lines = mutableListOf<WrappedLine>()
    var currentIndex = n
    while (currentIndex > 0) {
        val startIndex = breaks[currentIndex]
        val lineSyllables = syllableLayouts.subList(startIndex, currentIndex)
        val trimmedLine = trimDisplayLineTrailingSpaces(lineSyllables, nativeEngine, style, density)
        lines.add(0, trimmedLine)
        currentIndex = startIndex
    }

    return lines
}

/**
 * Calculates the final static layout positions for all syllables in the wrapped lines.
 * Handles alignment (Left/Right/RTL), row positioning, and relative offsets within the line.
 *
 * @param wrappedLines The lines after wrapping calculation.
 * @param isLineRightAligned Whether the entire block should be right-aligned.
 * @param canvasWidth Total width of the canvas.
 * @param lineHeight Height of a single line.
 * @param isRtl Whether the visual flow is RTL.
 * @return List of lists of [SyllableLayout], where each inner list is a row, with updated positions.
 */
fun calculateStaticLineLayout(
    wrappedLines: List<WrappedLine>,
    isLineRightAligned: Boolean,
    canvasWidth: Float,
    lineHeight: Float,
    isRtl: Boolean
): List<List<SyllableLayout>> {
    val layoutsByWord = mutableMapOf<Int, MutableList<SyllableLayout>>()

    val positionedLines = wrappedLines.mapIndexed { lineIndex, wrappedLine ->
        val maxBaselineInLine = wrappedLine.syllables.maxOfOrNull { it.firstBaseline } ?: 0f
        val rowTopY = lineIndex * lineHeight

        // 核心逻辑：如果是右对齐，起始点 = 画布宽 - 行宽。否则为 0。
        val startX = if (isLineRightAligned) {
            canvasWidth - wrappedLine.totalWidth
        } else {
            0f
        }

        // 下面的逻辑处理 RTL 单词内的排列，保持不变
        var currentX = if (isRtl) startX + wrappedLine.totalWidth else startX

        wrappedLine.syllables.map { initialLayout ->
            val positionX = if (isRtl) {
                currentX - initialLayout.width
            } else {
                currentX
            }
            val verticalOffset = maxBaselineInLine - initialLayout.firstBaseline
            val positionY = rowTopY + verticalOffset
            val positionedLayout = initialLayout.copy(position = Offset(positionX, positionY))
            layoutsByWord.getOrPut(positionedLayout.wordId) { mutableListOf() }
                .add(positionedLayout)

            if (isRtl) {
                currentX -= positionedLayout.width
            } else {
                currentX += positionedLayout.width
            }

            positionedLayout
        }
    }

    val animInfoByWord = mutableMapOf<Int, WordAnimationInfo>()
    val charOffsetsBySyllable = mutableMapOf<SyllableLayout, Int>()

    layoutsByWord.forEach { (wordId, layouts) ->
        if (layouts.first().useAwesomeAnimation) {
            animInfoByWord[wordId] = WordAnimationInfo(
                wordStartTime = layouts.minOf { it.syllable.start }.toLong(),
                wordEndTime = layouts.maxOf { it.syllable.end }.toLong(),
                wordContent = layouts.joinToString("") { it.syllable.content })
            var runningCharOffset = 0
            layouts.forEach { layout ->
                charOffsetsBySyllable[layout] = runningCharOffset
                runningCharOffset += layout.syllable.content.length
            }
        }
    }

    return positionedLines.map { line ->
        line.map { positionedLayout ->
            val wordLayouts = layoutsByWord.getValue(positionedLayout.wordId)
            val minX = wordLayouts.minOf { it.position.x }
            val maxX = wordLayouts.maxOf { it.position.x + it.width }
            val bottomY = wordLayouts.maxOf { it.position.y + it.layoutResult.size.height }

            positionedLayout.copy(
                wordPivot = Offset(x = (minX + maxX) / 2f, y = bottomY),
                wordAnimInfo = animInfoByWord[positionedLayout.wordId],
                charOffsetInWord = charOffsetsBySyllable[positionedLayout] ?: 0
            )
        }
    }
}

fun trimDisplayLineTrailingSpaces(
    displayLineSyllables: List<SyllableLayout>, nativeEngine: NativeTextEngine, style: TextStyle, density: Density
): WrappedLine {
    if (displayLineSyllables.isEmpty()) {
        return WrappedLine(emptyList(), 0f)
    }

    val processedSyllables = displayLineSyllables.toMutableList()
    var lastIndex = processedSyllables.lastIndex

    while (lastIndex >= 0 && processedSyllables[lastIndex].syllable.content.isBlank()) {
        processedSyllables.removeAt(lastIndex)
        lastIndex--
    }

    if (processedSyllables.isEmpty()) {
        return WrappedLine(emptyList(), 0f)
    }

    val lastLayout = processedSyllables.last()
    val originalContent = lastLayout.syllable.content
    val trimmedContent = originalContent.trimEnd()

    if (trimmedContent.length < originalContent.length) {
        if (trimmedContent.isNotEmpty()) {
            val fontSizePx = if (style.fontSize.isSp) style.fontSize.value * density.fontScale * density.density else style.fontSize.value
            val fontWeight = style.fontWeight?.weight?.toFloat() ?: 400f
            val trimmedJson = nativeEngine.processText(trimmedContent, fontSizePx, fontWeight)
            val trimmedLayoutResult = parseRustResult(trimmedJson)
            
            val trimmedLayout = lastLayout.copy(
                syllable = lastLayout.syllable.copy(content = trimmedContent),
                layoutResult = trimmedLayoutResult, // Fixed
                width = trimmedLayoutResult.total_width
            )
            processedSyllables[processedSyllables.lastIndex] = trimmedLayout
        } else {
            processedSyllables.removeAt(processedSyllables.lastIndex)
        }
    }

    val totalWidth = processedSyllables.sumOf { it.width.toDouble() }.toFloat()
    return WrappedLine(processedSyllables, totalWidth)
}
