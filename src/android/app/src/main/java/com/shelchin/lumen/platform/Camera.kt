package com.shelchin.lumen.platform

import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.CaptureResult
import android.hardware.camera2.TotalCaptureResult
import android.util.Range
import android.util.Size
import androidx.activity.ComponentActivity
import androidx.annotation.OptIn
import androidx.camera.camera2.interop.Camera2CameraControl
import androidx.camera.camera2.interop.Camera2CameraInfo
import androidx.camera.camera2.interop.Camera2Interop
import androidx.camera.camera2.interop.CaptureRequestOptions
import androidx.camera.camera2.interop.ExperimentalCamera2Interop
import androidx.camera.core.Camera
import androidx.camera.core.CameraInfo
import androidx.camera.core.CameraSelector
import androidx.camera.core.FocusMeteringAction
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.core.SurfaceOrientedMeteringPointFactory
import androidx.camera.core.resolutionselector.AspectRatioStrategy
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import com.shelchin.lumen.BuildConfig
import com.sun.jna.Native
import com.sun.jna.Pointer
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import uniffi.lumen_ffi.CameraFrame
import uniffi.lumen_ffi.FrameFormat
import uniffi.lumen_ffi.LumenCore
import java.io.File
import java.nio.ByteBuffer
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import kotlin.math.pow
import kotlin.math.roundToInt

/**
 * The camera, feeding the core's scan pipeline.
 *
 * Frames arrive as YUV_420_888 straight from the sensor: no conversion to RGB, no copy. The
 * core reads the planes in place through their addresses during the analyzer callback,
 * scales only the region around the code, and decides itself which frames to scan (locked
 * to the sender's frame rate), so the analyzer does almost nothing per frame.
 *
 * The frame rate is the core's call too: it starts at a steady 30 and asks for 60 when the
 * sender plays fast enough, if this camera offers 60 to ordinary capture sessions (many phones
 * keep their 120/240 fps modes for video recording only).
 *
 * Exposure is ours, not auto exposure's. A screen is a light source showing a new picture every
 * few milliseconds: auto exposure meters it like scenery (1/50 s at ISO 100 was measured), which
 * clips the bright colours and blends consecutive frames into pastels nothing can decode. Here
 * the exposure stays under a sixth of the sender's frame (1/400 s until the core knows it) and
 * the gain follows the brightest pixels of the code, so they sit just under clipping.
 */
@OptIn(ExperimentalCamera2Interop::class)
class LumenCamera(private val activity: ComponentActivity, private val core: LumenCore) {
    val previewView: PreviewView = PreviewView(activity).apply {
        implementationMode = PreviewView.ImplementationMode.PERFORMANCE
        scaleType = PreviewView.ScaleType.FILL_CENTER
    }
    private val executor = Executors.newSingleThreadExecutor { r -> Thread(r, "lumen-camera").apply { priority = Thread.NORM_PRIORITY + 1 } }
    private var provider: ProcessCameraProvider? = null
    private var camera: Camera? = null
    private var ranges: List<Range<Int>> = emptyList()

    @Volatile private var readout = 0f
    private var analysis: ImageAnalysis? = null

    // exposure (see the class comment); `lock` guards it and the frame-rate band
    private val lock = Any()
    private var fpsRange: Range<Int>? = null
    private var manualOk = false
    private var manual = false
    private var isoRange = Range(100, 800)
    private var expRange = Range(100_000L, 33_000_000L)
    /** Exposure time × ISO for the wanted brightness (ns·ISO). */
    private var product = 0.0
    private var expNs = 0L
    /** The longest exposure allowed (the core's, from the sender's frame rate). */
    private var capNs = 2_500_000.0
    private var iso = 0
    private var aeFrames = 0
    private var meterCount = 0

    /** Where the code is in camera-frame pixels (x0, y0, x1, y1), while it is tracked. */
    @Volatile private var codeBox: FloatArray? = null
    private var focusedAt = 0L

    // debug builds: what the camera does, for `watch`, and raw frames for `grab`
    @Volatile private var meta = ""
    @Volatile private var frameInfo = 0L
    @Volatile private var grabLeft = 0
    private var grabbed = 0

    /**
     * The sensor's readout time, for the core: many phone sensors take most of a frame to read
     * their rows, so each capture holds parts of two sender frames, and the receiver must scan
     * every capture to see each frame whole.
     */
    private val captureCallback = object : CameraCaptureSession.CaptureCallback() {
        override fun onCaptureCompleted(session: CameraCaptureSession, request: CaptureRequest, result: TotalCaptureResult) {
            val skew = (result.get(CaptureResult.SENSOR_ROLLING_SHUTTER_SKEW) ?: 0L) / 1e6f
            if (kotlin.math.abs(skew - readout) > 0.5f) {
                readout = skew
                core.cameraReadout(skew)
            }
            val exp = result.get(CaptureResult.SENSOR_EXPOSURE_TIME) ?: 0L
            val iso = result.get(CaptureResult.SENSOR_SENSITIVITY) ?: 0
            if (manualOk && !manual) {
                // start from what auto exposure found, then meter ourselves
                val ae = result.get(CaptureResult.CONTROL_AE_STATE)
                if (exp > 0 && iso > 0 && (ae == CaptureResult.CONTROL_AE_STATE_CONVERGED || ++aeFrames > 20)) {
                    synchronized(lock) {
                        product = exp.toDouble() * iso * 0.5
                        manual = true
                        settle()
                    }
                    apply()
                }
            }
            if (!BuildConfig.SCRIPTS) return
            val fd = result.get(CaptureResult.SENSOR_FRAME_DURATION) ?: 0L
            val focus = result.get(CaptureResult.LENS_FOCUS_DISTANCE) ?: 0f
            val af = result.get(CaptureResult.CONTROL_AF_STATE) ?: -1
            val ae = result.get(CaptureResult.CONTROL_AE_STATE) ?: -1
            meta = "exp 1/%.0f iso %d fd %.1fms skew %.1fms focus %.1fD af %d ae %d".format(1e9 / exp.coerceAtLeast(1), iso, fd / 1e6, skew, focus, af, ae)
        }
    }

    /** Debug: the camera's last capture settings and the frame size. */
    fun stats(): String {
        val f = frameInfo
        return "${f shr 32}x${(f shr 16) and 0xffff} r${(f shr 4) and 0xfff} uv${f and 0xf} $meta"
    }

    /** Debug: writes the next `n` frames to files/frame-N.nv12 (the iOS dump format). */
    fun grab(n: Int) {
        grabLeft = n
    }

    /** Whether the picture is mirrored (front camera), for the outline overlay. */
    var mirrored by mutableStateOf(false)
        private set

    /** Back cameras first, then front. */
    private fun cameras(p: ProcessCameraProvider): List<CameraInfo> {
        val infos = p.availableCameraInfos
        return infos.filter { it.lensFacing == CameraSelector.LENS_FACING_BACK } + infos.filter { it.lensFacing == CameraSelector.LENS_FACING_FRONT }
    }

    /** The auto-exposure frame-rate ranges this camera allows. */
    private fun fpsRanges(info: CameraInfo): List<Range<Int>> =
        runCatching { Camera2CameraInfo.from(info).getCameraCharacteristic(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES)?.toList() }
            .getOrNull().orEmpty()

    /** A steady `fps` if the camera has it, else the range up to it that dips the least. */
    private fun rangeFor(fps: Int): Range<Int>? =
        ranges.firstOrNull { it.lower == fps && it.upper == fps } ?: ranges.filter { it.upper == fps }.maxByOrNull { it.lower }

    private fun maxFps(): Int = if (rangeFor(60) != null) 60 else 30

    fun start(index: Int, answer: (JsonObject) -> Unit) {
        val future = ProcessCameraProvider.getInstance(activity)
        future.addListener({
            val p = runCatching { future.get() }.getOrNull()
            if (p == null) {
                answer(fail("unavailable"))
                return@addListener
            }
            provider = p
            val all = cameras(p)
            if (all.isEmpty()) {
                answer(fail("unavailable"))
                return@addListener
            }
            val info = all[index.mod(all.size)]
            val selector = CameraSelector.Builder().addCameraFilter { list -> list.filter { it == info } }.build()
            ranges = fpsRanges(info)
            val start = rangeFor(30)
            // 1080p, 16:9 for both use cases: the preview shows what the scanner sees
            val res = ResolutionSelector.Builder()
                .setAspectRatioStrategy(AspectRatioStrategy.RATIO_16_9_FALLBACK_AUTO_STRATEGY)
                .setResolutionStrategy(ResolutionStrategy(Size(1920, 1080), ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER))
                .build()
            val preview = Preview.Builder()
                .setResolutionSelector(res)
                .also { b -> start?.let { Camera2Interop.Extender(b).setCaptureRequestOption(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, it) } }
                .build()
                .also { it.surfaceProvider = previewView.surfaceProvider }
            val analysis = ImageAnalysis.Builder()
                .setResolutionSelector(res)
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_YUV_420_888)
                .also { b -> start?.let { Camera2Interop.Extender(b).setCaptureRequestOption(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, it) } }
                .also { b -> Camera2Interop.Extender(b).setSessionCaptureCallback(captureCallback) }
                .build()
            analysis.setAnalyzer(executor) { image ->
                try {
                    val pl = image.planes
                    val y = pl[0]
                    val u = pl[1]
                    val v = pl[2]
                    if (manual && ++meterCount % 3 == 0) meter(y.buffer, y.rowStride, image.width, image.height)
                    if (BuildConfig.SCRIPTS) {
                        frameInfo = (image.width.toLong() shl 32) or (image.height.toLong() shl 16) or (image.imageInfo.rotationDegrees.toLong() shl 4) or u.pixelStride.toLong()
                        if (grabLeft > 0) {
                            grabLeft--
                            dump(image.width, image.height, y.buffer, y.rowStride, u.buffer, v.buffer, u.rowStride)
                        }
                    }
                    core.cameraFrame(
                        CameraFrame(
                            width = image.width.toUInt(),
                            height = image.height.toUInt(),
                            format = FrameFormat.YUV420,
                            rotation = image.imageInfo.rotationDegrees.toUInt(),
                            plane0 = address(y.buffer), len0 = y.buffer.capacity().toULong(), rowStride0 = y.rowStride.toUInt(),
                            plane1 = address(u.buffer), len1 = u.buffer.capacity().toULong(), rowStride1 = u.rowStride.toUInt(),
                            pixelStride1 = u.pixelStride.toUInt(),
                            plane2 = address(v.buffer), len2 = v.buffer.capacity().toULong(), rowStride2 = v.rowStride.toUInt(),
                            pixelStride2 = v.pixelStride.toUInt(),
                        )
                    )
                } finally {
                    image.close()
                }
            }
            val cam = runCatching {
                p.unbindAll()
                p.bindToLifecycle(activity, selector, preview, analysis)
            }.getOrElse {
                answer(fail("failed", it.message))
                return@addListener
            }
            camera = cam
            this.analysis = analysis
            mirrored = cam.cameraInfo.lensFacing == CameraSelector.LENS_FACING_FRONT
            prepareExposure(info, start)
            tune(cam.cameraInfo, cam)
            val zs = cam.cameraInfo.zoomState.value
            val zMin = zs?.minZoomRatio ?: 1f
            val zMax = minOf(zs?.maxZoomRatio ?: 1f, zMin * 8f)
            answer(buildJsonObject {
                put("type", "camera"); put("ok", true)
                put("zoomMin", zMin); put("zoomMax", zMax); put("zoom", zs?.zoomRatio ?: zMin)
                put("devices", all.size)
                put("maxFps", maxFps()); put("uhd", false)
            })
        }, ContextCompat.getMainExecutor(activity))
    }

    /** A screen is a light source: auto exposure clips its brightest colours. Half a stop under. */
    private fun tune(info: CameraInfo, cam: Camera) {
        val ex = info.exposureState
        if (ex.isExposureCompensationSupported) {
            val step = ex.exposureCompensationStep.toFloat()
            if (step > 0f) {
                val idx = (-0.5f / step).roundToInt().coerceIn(ex.exposureCompensationRange.lower, ex.exposureCompensationRange.upper)
                cam.cameraControl.setExposureCompensationIndex(idx)
            }
        }
    }

    fun stop() {
        provider?.unbindAll()
        camera = null
        analysis = null
        codeBox = null
        readout = 0f
        core.cameraReadout(0f)
    }

    fun zoom(level: Float) {
        camera?.cameraControl?.setZoomRatio(level)
    }

    /** The core's frame-rate band; changed on the running session, without rebinding. */
    fun setMode(fps: Int) {
        val range = rangeFor(fps.coerceAtMost(maxFps())) ?: return
        synchronized(lock) {
            fpsRange = range
            settle()
        }
        apply()
    }

    // ---------------------------------------------------------------- exposure

    private fun prepareExposure(info: CameraInfo, start: Range<Int>?) {
        val c2 = Camera2CameraInfo.from(info)
        val caps = c2.getCameraCharacteristic(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES)
        synchronized(lock) {
            fpsRange = start
            manual = false
            aeFrames = 0
            manualOk = caps?.contains(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES_MANUAL_SENSOR) == true
            c2.getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_SENSITIVITY_RANGE)?.let { isoRange = it }
            c2.getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_EXPOSURE_TIME_RANGE)?.let { expRange = it }
        }
    }

    private fun frameNs(): Long = 1_000_000_000L / (fpsRange?.upper ?: 30)

    /** The core's exposure limit (µs): a sixth of the sender's frame period. */
    fun setExposureCap(us: Int) {
        synchronized(lock) {
            capNs = us.coerceIn(500, 16_000) * 1000.0
            settle()
        }
        apply()
    }

    /** Exposure time and ISO for `product`: as long as the cap allows, gain for the rest. */
    private fun settle() {
        val isoMin = isoRange.lower.toDouble()
        val isoMax = minOf(isoRange.upper, 3200).toDouble()
        product = product.coerceIn(expRange.lower * isoMin, frameNs() * 0.9 * isoMax)
        var t = minOf(capNs, product / isoMin)
        if (product / t > isoMax) t = product / isoMax // too dark for the cap even at full gain
        expNs = t.toLong().coerceIn(expRange.lower, expRange.upper)
        iso = (product / expNs).toInt().coerceIn(isoRange.lower, isoRange.upper)
    }

    /** All the session options this class sets, at once (a call replaces the previous set). */
    private fun apply() {
        val cam = camera ?: return
        val b = CaptureRequestOptions.Builder()
        synchronized(lock) {
            fpsRange?.let { b.setCaptureRequestOption(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, it) }
            if (manual) {
                b.setCaptureRequestOption(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_OFF)
                b.setCaptureRequestOption(CaptureRequest.SENSOR_EXPOSURE_TIME, expNs)
                b.setCaptureRequestOption(CaptureRequest.SENSOR_SENSITIVITY, iso)
                b.setCaptureRequestOption(CaptureRequest.SENSOR_FRAME_DURATION, frameNs())
            }
        }
        Camera2CameraControl.from(cam.cameraControl).setCaptureRequestOptions(b.build())
    }

    /**
     * Brightness control on the luma plane: the 99th percentile of the code (or of the middle
     * of the picture while there is none) aims just under clipping. Every third frame, damped:
     * the camera applies a change a few frames later.
     */
    private fun meter(y: ByteBuffer, stride: Int, w: Int, h: Int) {
        val box = codeBox
        val (x0, y0, x1, y1) = if (box != null) {
            listOf(box[0].toInt().coerceIn(0, w - 1), box[1].toInt().coerceIn(0, h - 1), box[2].toInt().coerceIn(1, w), box[3].toInt().coerceIn(1, h))
        } else {
            listOf(w / 5, h / 5, w * 4 / 5, h * 4 / 5)
        }
        if (x1 - x0 < 16 || y1 - y0 < 16) return
        val hist = IntArray(256)
        val step = maxOf(4, minOf(x1 - x0, y1 - y0) / 96)
        var n = 0
        var yy = y0
        while (yy < y1) {
            val row = yy * stride
            var xx = x0
            while (xx < x1) {
                hist[y.get(row + xx).toInt() and 0xff]++
                n++
                xx += step
            }
            yy += step
        }
        var acc = 0
        var p99 = 255
        for (v in 255 downTo 0) {
            acc += hist[v]
            if (acc * 100 >= n) {
                p99 = v
                break
            }
        }
        var ratio = 228.0 / p99.coerceAtLeast(8)
        if (p99 >= 250) ratio = minOf(ratio, 0.6) // clipped: how far is unknown, step down hard
        ratio = ratio.pow(0.6).coerceIn(0.5, 1.6)
        if (kotlin.math.abs(ratio - 1.0) < 0.05) return
        synchronized(lock) {
            product *= ratio
            settle()
        }
        apply()
    }

    // ---------------------------------------------------------------- the code's position

    /**
     * The code's corners in camera-frame pixels (null: not in view). Steers the metering and,
     * when the code appears or every few seconds while it stays, focuses on it.
     */
    fun track(points: List<List<Float>>?, frameW: Int, frameH: Int) {
        if (points == null || points.size != 4) {
            codeBox = null
            return
        }
        val xs = points.map { it[0] }
        val ys = points.map { it[1] }
        val box = floatArrayOf(xs.min(), ys.min(), xs.max(), ys.max())
        val appeared = codeBox == null
        codeBox = box
        val now = System.currentTimeMillis()
        if (!appeared && now - focusedAt < 3000) return
        val cam = camera ?: return
        val a = analysis ?: return
        focusedAt = now
        val f = SurfaceOrientedMeteringPointFactory(frameW.toFloat(), frameH.toFloat(), a)
        val point = f.createPoint((box[0] + box[2]) / 2, (box[1] + box[3]) / 2, 0.25f)
        val action = FocusMeteringAction.Builder(point, FocusMeteringAction.FLAG_AF).setAutoCancelDuration(3500, TimeUnit.MILLISECONDS).build()
        runCatching { cam.cameraControl.startFocusAndMetering(action) }
    }

    /** What the camera allows, for debug scripts. */
    fun describe(): String = "ranges ${ranges.joinToString { "[${it.lower},${it.upper}]" }} · max ${maxFps()} fps"

    private fun fail(error: String, message: String? = null) = buildJsonObject {
        put("type", "camera"); put("ok", false); put("error", error)
        if (message != null) put("message", message)
    }

    /** NV12 with a 16-byte header (width, height, y stride, uv stride), like iOS `grab`. */
    private fun dump(w: Int, h: Int, y: ByteBuffer, yStride: Int, u: ByteBuffer, v: ByteBuffer, uvStride: Int) {
        // semi-planar either way: Cb first is whichever plane starts earlier in memory
        val cbFirst = address(u) < address(v)
        val uv = if (cbFirst) u else v
        val head = ByteBuffer.allocate(16).order(java.nio.ByteOrder.LITTLE_ENDIAN).putInt(w).putInt(h).putInt(yStride).putInt(uvStride).array()
        val yb = ByteArray(yStride * h)
        y.duplicate().apply { rewind() }.get(yb, 0, minOf(yb.size, y.capacity()))
        val cb = ByteArray(uvStride * (h / 2))
        uv.duplicate().apply { rewind() }.get(cb, 0, minOf(cb.size, uv.capacity()))
        if (!cbFirst) {
            // V then U in memory: swap each pair to get Cb, Cr order
            var i = 0
            while (i + 1 < cb.size) { val t = cb[i]; cb[i] = cb[i + 1]; cb[i + 1] = t; i += 2 }
        }
        grabbed++
        File(activity.filesDir, "frame-$grabbed.nv12").outputStream().use { it.write(head); it.write(yb); it.write(cb) }
    }

    private fun address(b: ByteBuffer): ULong = Pointer.nativeValue(Native.getDirectBufferPointer(b)).toULong()
}
