package app.takedock.observer;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.Instrumentation;
import android.app.UiAutomation;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.res.Resources;
import android.os.Bundle;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import org.json.JSONObject;

/** Passive, event-driven observation; never connects with zero UiAutomation flags. */
public final class ObserverInstrumentation extends Instrumentation {
    private static final String CAMERA = "net.sourceforge.opencamera";
    private final CountDownLatch stopped = new CountDownLatch(1);
    private String session;
    private volatile long eventTime;
    private long sequence;
    private UiAutomation automation;
    private HandlerThread readerThread;
    private Handler reader;
    private CameraStateReader.Labels labels;
    private CameraStateReader.Snapshot previous;
    private BroadcastReceiver stopReceiver;

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        session = arguments == null ? null : arguments.getString("session");
        if (session == null) session = UUID.randomUUID().toString();
        start();
    }

    @Override public void onStart() {
        try {
            Context context = getTargetContext();
            Resources resources = context.createPackageContext(CAMERA, 0).getResources();
            labels = new CameraStateReader.Labels(label(resources, "start_video"), label(resources, "stop_video"), label(resources, "pause_video"), label(resources, "resume_video"));
            automation = getUiAutomation(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES);
            if (automation == null) throw new IllegalStateException("UiAutomation unavailable");
            AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
            info.eventTypes = AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED | AccessibilityEvent.TYPE_WINDOWS_CHANGED | AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED;
            automation.setServiceInfo(info);
            readerThread = new HandlerThread("TakeDock-state-reader");
            readerThread.start();
            reader = new Handler(readerThread.getLooper());
            stopReceiver = new BroadcastReceiver() {
                @Override public void onReceive(Context receiverContext, Intent intent) {
                    if (session.equals(intent.getStringExtra("session"))) stopped.countDown();
                }
            };
            context.registerReceiver(stopReceiver, new IntentFilter("app.takedock.observer.STOP"), "android.permission.DUMP", null, Context.RECEIVER_EXPORTED);
            Runnable snapshot = this::readAndEmit;
            automation.setOnAccessibilityEventListener(event -> {
                if (event.getEventType() == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED && !CAMERA.contentEquals(event.getPackageName() == null ? "" : event.getPackageName())) return;
                eventTime = event.getEventTime();
                reader.removeCallbacks(snapshot);
                reader.postDelayed(snapshot, 8);
            });
            emit(new JSONObject().put("type", "ready"));
            eventTime = SystemClock.uptimeMillis();
            reader.post(snapshot);
            stopped.await();
        } catch (Exception error) {
            try { emit(new JSONObject().put("type", "error").put("message", error.getClass().getSimpleName() + ": " + error.getMessage())); }
            catch (Exception ignored) { /* No unsafe connection fallback. */ }
        } finally {
            if (automation != null) automation.setOnAccessibilityEventListener(null);
            if (readerThread != null) {
                readerThread.quitSafely();
                try { readerThread.join(1500); } catch (InterruptedException interrupted) { Thread.currentThread().interrupt(); }
            }
            if (stopReceiver != null) getTargetContext().unregisterReceiver(stopReceiver);
            // Instrumentation.finish() releases its owned UiAutomation connection.
            finish(0, new Bundle());
        }
    }

    private static String label(Resources resources, String name) {
        int id = resources.getIdentifier(name, "string", CAMERA);
        if (id == 0) throw new IllegalStateException("Unsupported Open Camera resource: " + name);
        return resources.getString(id);
    }

    private void readAndEmit() {
        try {
            AccessibilityNodeInfo root = automation.getRootInActiveWindow();
            String packageName = root == null || root.getPackageName() == null ? "" : root.getPackageName().toString();
            List<CameraStateReader.Node> nodes = new ArrayList<>();
            if (CAMERA.equals(packageName)) {
                ArrayDeque<AccessibilityNodeInfo> queue = new ArrayDeque<>();
                queue.add(root);
                for (int visited = 0; !queue.isEmpty() && visited < 512; visited++) {
                    AccessibilityNodeInfo node = queue.removeFirst();
                    String id = node.getViewIdResourceName();
                    if ((CAMERA + ":id/take_photo").equals(id) || (CAMERA + ":id/pause_video").equals(id)) {
                        nodes.add(new CameraStateReader.Node(id, node.getContentDescription() == null ? "" : node.getContentDescription().toString(), node.isVisibleToUser()));
                    }
                    for (int i = 0; i < node.getChildCount(); i++) {
                        AccessibilityNodeInfo child = node.getChild(i);
                        if (child != null) queue.addLast(child);
                    }
                }
            }
            CameraStateReader.Snapshot state = CameraStateReader.read(packageName, nodes, labels);
            if (state.equals(previous)) return;
            previous = state;
            emit(new JSONObject().put("type", "state").put("seq", ++sequence).put("event_time", eventTime)
                .put("foreground", state.foreground()).put("video_mode", state.videoMode()).put("state", state.state().name().toLowerCase(java.util.Locale.ROOT)));
        } catch (Exception error) {
            try { emit(new JSONObject().put("type", "error").put("message", "State observation failed: " + error.getClass().getSimpleName())); }
            catch (Exception ignored) { /* Transport already unavailable. */ }
        }
    }

    private synchronized void emit(JSONObject message) throws org.json.JSONException {
        message.put("version", 1).put("session", session);
        Bundle status = new Bundle();
        status.putString("takedock", message.toString());
        sendStatus(0, status);
    }
}
