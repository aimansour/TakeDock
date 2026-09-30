package app.takedock.observer;

import java.util.List;

public final class CameraStateReader {
    public enum State { UNKNOWN, IDLE, RECORDING, PAUSED }
    public record Node(String id, String description, boolean visible) {}
    public record Snapshot(boolean foreground, boolean videoMode, State state) {}
    public record Labels(String start, String stop, String pause, String resume) {}
    public static final Labels ENGLISH = new Labels("Start recording video", "Stop recording video", "Pause video recording", "Resume video recording");

    public static Snapshot read(String packageName, List<Node> nodes) {
        return read(packageName, nodes, ENGLISH);
    }
    public static Snapshot read(String packageName, List<Node> nodes, Labels labels) {
        if (!"net.sourceforge.opencamera".equals(packageName)) return new Snapshot(false, false, State.UNKNOWN);
        String shutter = "", pause = "";
        for (Node node : nodes) {
            if (!node.visible() || node.description() == null) continue;
            if ("net.sourceforge.opencamera:id/take_photo".equals(node.id())) shutter = node.description();
            if ("net.sourceforge.opencamera:id/pause_video".equals(node.id())) pause = node.description();
        }
        if (labels.start().equals(shutter)) return new Snapshot(true, true, State.IDLE);
        if (labels.stop().equals(shutter)) {
            State state = labels.pause().equals(pause) ? State.RECORDING : labels.resume().equals(pause) ? State.PAUSED : State.UNKNOWN;
            return new Snapshot(true, true, state);
        }
        return new Snapshot(true, false, State.UNKNOWN);
    }
}
