package `in`.minori.key.phase0

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorFilter
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.RippleDrawable
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView

/** Native Android Liquid Glass-style surfaces. No Apple/private API or UI SDK. */
internal class GlassSurface(private val density: Float, private val action: Boolean = false) : Drawable() {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    override fun draw(canvas: Canvas) {
        val rect = RectF(bounds)
        val radius = density * if (action) 28f else 30f
        // A dark frosted base keeps white text legible across the luminous backdrop.
        paint.style = Paint.Style.FILL
        paint.shader = LinearGradient(rect.left, rect.top, rect.right, rect.bottom,
            intArrayOf(Color.argb(242, 32, 65, 78), Color.argb(237, 14, 35, 52)), null, Shader.TileMode.CLAMP)
        canvas.drawRoundRect(rect, radius, radius, paint)
        paint.shader = RadialGradient(rect.left + rect.width() * .2f, rect.top,
            rect.width().coerceAtLeast(1f), intArrayOf(Color.argb(42, 174, 255, 233), Color.TRANSPARENT), null, Shader.TileMode.CLAMP)
        canvas.drawRoundRect(rect, radius, radius, paint)
        // Reflected rim, with a brighter upper edge and restrained lower edge.
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = density
        paint.shader = LinearGradient(rect.left, rect.top, rect.right, rect.bottom,
            intArrayOf(Color.argb(170, 226, 255, 249), Color.argb(20, 214, 255, 247), Color.argb(80, 126, 217, 221)),
            floatArrayOf(0f, .6f, 1f), Shader.TileMode.CLAMP)
        rect.inset(density, density)
        canvas.drawRoundRect(rect, radius, radius, paint)
        paint.shader = null
    }
    override fun setAlpha(alpha: Int) = Unit
    override fun setColorFilter(colorFilter: ColorFilter?) = Unit
    @Deprecated("Drawable opacity is not used for composition")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}

internal class GlassBackdrop : Drawable() {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    override fun draw(canvas: Canvas) {
        val r = RectF(bounds)
        paint.shader = LinearGradient(0f, 0f, r.width(), r.height(),
            intArrayOf(Color.rgb(8, 25, 43), Color.rgb(14, 62, 72), Color.rgb(23, 35, 66)),
            null, Shader.TileMode.CLAMP)
        canvas.drawRect(r, paint)
        paint.shader = RadialGradient(r.width() * .9f, r.height() * .12f,
            (r.width() * .8f).coerceAtLeast(1f), Color.argb(135, 50, 190, 175), Color.TRANSPARENT, Shader.TileMode.CLAMP)
        canvas.drawRect(r, paint)
        paint.shader = RadialGradient(r.width() * .05f, r.height() * .8f,
            r.width().coerceAtLeast(1f), Color.argb(85, 99, 111, 213), Color.TRANSPARENT, Shader.TileMode.CLAMP)
        canvas.drawRect(r, paint)
        paint.shader = null
    }
    override fun setAlpha(alpha: Int) = Unit
    override fun setColorFilter(colorFilter: ColorFilter?) = Unit
    @Deprecated("Drawable opacity is not used for composition")
    override fun getOpacity(): Int = PixelFormat.OPAQUE
}

internal object GlassUI {
    fun dp(context: Context, value: Int) = (value * context.resources.displayMetrics.density).toInt()
    fun panel(context: Context) = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        val padding = dp(context, 22)
        setPadding(padding, padding, padding, padding)
        background = GlassSurface(resources.displayMetrics.density)
        layoutParams = LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(context, 18) }
    }
    fun text(context: Context, value: String, size: Float = 17f, heading: Boolean = false) = TextView(context).apply {
        text = value
        textSize = size
        setTextColor(Color.rgb(244, 251, 255))
        setLineSpacing(dp(context, 5).toFloat(), 1f)
        if (heading) {
            typeface = Typeface.create("sans-serif-medium", Typeface.BOLD)
            isAccessibilityHeading = true
        }
        layoutParams = LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(context, 12) }
    }
    fun button(context: Context, title: String, onClick: () -> Unit) = Button(context).apply {
        text = title
        textSize = 16f
        isAllCaps = false
        setTextColor(Color.WHITE)
        minHeight = dp(context, 56)
        minimumHeight = dp(context, 56)
        setPadding(dp(context, 18), dp(context, 12), dp(context, 18), dp(context, 12))
        background = RippleDrawable(android.content.res.ColorStateList.valueOf(Color.argb(60, 213, 255, 245)),
            GlassSurface(resources.displayMetrics.density, true), null)
        layoutParams = LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(context, 12) }
        setOnClickListener { onClick() }
    }
}
